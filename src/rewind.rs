//! Turn-scoped rewind of file mutations.
//!
//! The unit is the turn: one user message and every tool call the agent made
//! answering it. Before `edit`, `write` or `hashline_edit` mutates a path, the
//! pre-image of that path is copied into the store unless the current turn
//! already holds one. A path the turn creates records a tombstone, so restore
//! deletes it rather than leaving it behind.
//!
//! Blobs are content-addressed, so a file re-touched across five turns costs
//! one copy per distinct content.
//!
//! Bash is not covered and cannot be. A turn that ran bash records the fact so
//! the surface can say the file restore is partial.

use crate::config::Config;
use crate::error::{Error, Result};
use crate::extensions::safe_canonicalize;
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

pub const DEFAULT_MAX_TURNS: usize = 50;
pub const DEFAULT_MAX_BYTES: u64 = 256 * 1024 * 1024;

/// Retention bounds. Whichever binds first wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    pub max_turns: usize,
    pub max_bytes: u64,
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            max_turns: DEFAULT_MAX_TURNS,
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

/// One turn's pre-images. `None` blob means the path did not exist when the
/// turn started, so restoring the turn deletes it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnManifest {
    pub turn: u64,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub ran_bash: bool,
    #[serde(default)]
    pub entries: BTreeMap<String, Option<String>>,
}

/// What the rewind surface renders per turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnSummary {
    pub turn: u64,
    pub message: String,
    pub files_changed: usize,
    pub ran_bash: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub entry: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RestoreReport {
    pub restored: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
    pub refused: Vec<Refusal>,
}

impl RestoreReport {
    pub fn changed(&self) -> usize {
        self.restored.len() + self.deleted.len()
    }
}

fn io_err(err: std::io::Error) -> Error {
    Error::Io(Box::new(err))
}

fn json_err(err: serde_json::Error) -> Error {
    Error::Json(Box::new(err))
}

/// Session ids reach the filesystem as a directory name, so strip anything
/// that could climb out of the store root.
fn sanitize_session_id(session_id: &str) -> String {
    let mut out = String::with_capacity(session_id.len());
    for ch in session_id.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('.');
    if trimmed.is_empty() {
        "unnamed".to_string()
    } else {
        trimmed.to_string()
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn is_blob_name(name: &str) -> bool {
    name.len() == 64 && name.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Workspace-relative key with forward slashes, so a manifest written on one
/// platform reads on another.
fn relative_key(rel: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

pub struct RewindStore {
    root: PathBuf,
    workspace: PathBuf,
    retention: Retention,
    turns: Vec<TurnManifest>,
    active: Option<usize>,
}

impl RewindStore {
    /// `~/.kesa/agent/rewind/<session-id>/`.
    pub fn base_dir() -> PathBuf {
        Config::global_dir().join("rewind")
    }

    pub fn open(session_id: &str, workspace: &Path) -> Result<Self> {
        let root = Self::base_dir().join(sanitize_session_id(session_id));
        Self::open_at(root, workspace, Retention::default())
    }

    pub fn open_at(root: PathBuf, workspace: &Path, retention: Retention) -> Result<Self> {
        let workspace = safe_canonicalize(workspace);
        std::fs::create_dir_all(root.join("blobs")).map_err(io_err)?;
        std::fs::create_dir_all(root.join("turns")).map_err(io_err)?;
        let mut store = Self {
            root,
            workspace,
            retention,
            turns: Vec::new(),
            active: None,
        };
        store.load_turns()?;
        store.gc()?;
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    pub fn turns(&self) -> &[TurnManifest] {
        &self.turns
    }

    pub fn summaries(&self) -> Vec<TurnSummary> {
        self.turns
            .iter()
            .map(|turn| TurnSummary {
                turn: turn.turn,
                message: turn.message.clone(),
                files_changed: turn.entries.len(),
                ran_bash: turn.ran_bash,
            })
            .collect()
    }

    fn turns_dir(&self) -> PathBuf {
        self.root.join("turns")
    }

    fn blobs_dir(&self) -> PathBuf {
        self.root.join("blobs")
    }

    fn manifest_path(&self, turn: u64) -> PathBuf {
        self.turns_dir().join(format!("{turn:020}.json"))
    }

    fn load_turns(&mut self) -> Result<()> {
        let mut turns = Vec::new();
        let entries = match std::fs::read_dir(self.turns_dir()) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(io_err(err)),
        };
        for entry in entries {
            let entry = entry.map_err(io_err)?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let raw = std::fs::read(&path).map_err(io_err)?;
            let manifest: TurnManifest = serde_json::from_slice(&raw).map_err(json_err)?;
            turns.push(manifest);
        }
        turns.sort_by_key(|turn| turn.turn);
        self.turns = turns;
        self.active = None;
        Ok(())
    }

    /// Start a new turn. Every capture after this lands in it.
    pub fn begin_turn(&mut self, message: &str) -> Result<u64> {
        let next = self.turns.last().map_or(1, |turn| turn.turn + 1);
        self.turns.push(TurnManifest {
            turn: next,
            message: message.to_string(),
            ran_bash: false,
            entries: BTreeMap::new(),
        });
        self.active = Some(self.turns.len() - 1);
        self.persist_active()?;
        Ok(next)
    }

    fn active_index(&mut self) -> Result<usize> {
        if let Some(index) = self.active {
            return Ok(index);
        }
        self.begin_turn("")?;
        self.active
            .ok_or_else(|| Error::validation("rewind: no active turn after begin_turn"))
    }

    /// A turn that ran bash is only partly covered by a file restore.
    pub fn note_bash(&mut self) -> Result<()> {
        let index = self.active_index()?;
        if self.turns[index].ran_bash {
            return Ok(());
        }
        self.turns[index].ran_bash = true;
        self.persist_active()
    }

    pub fn active_turn(&self) -> Option<u64> {
        self.active.map(|index| self.turns[index].turn)
    }

    /// Copy the pre-image of `path` into the store unless this turn already has
    /// one. Returns whether a new pre-image was recorded.
    pub fn capture(&mut self, path: &Path) -> Result<bool> {
        let canonical = safe_canonicalize(path);
        if canonical.starts_with(&self.root) {
            return Ok(false);
        }
        let Ok(rel) = canonical.strip_prefix(&self.workspace) else {
            return Ok(false);
        };
        let Some(key) = relative_key(rel) else {
            return Ok(false);
        };

        let index = self.active_index()?;
        if self.turns[index].entries.contains_key(&key) {
            return Ok(false);
        }

        let blob = match std::fs::read(&canonical) {
            Ok(bytes) => Some(self.write_blob(&bytes)?),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => return Err(io_err(err)),
        };
        self.turns[index].entries.insert(key, blob);
        self.persist_active().map(|()| true).map_err(|err| {
            self.turns[index].entries.clear();
            err
        })
    }

    fn write_blob(&self, bytes: &[u8]) -> Result<String> {
        let digest = sha2::Sha256::digest(bytes);
        let name = hex(&digest);
        let target = self.blobs_dir().join(&name);
        if target.exists() {
            return Ok(name);
        }
        let temp = self.blobs_dir().join(format!("{name}.tmp"));
        std::fs::write(&temp, bytes).map_err(io_err)?;
        std::fs::rename(&temp, &target).map_err(io_err)?;
        Ok(name)
    }

    fn persist_active(&self) -> Result<()> {
        let Some(index) = self.active else {
            return Ok(());
        };
        self.persist(index)
    }

    fn persist(&self, index: usize) -> Result<()> {
        let manifest = &self.turns[index];
        let path = self.manifest_path(manifest.turn);
        let body = serde_json::to_vec_pretty(manifest).map_err(json_err)?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, &body).map_err(io_err)?;
        std::fs::rename(&temp, &path).map_err(io_err)
    }

    /// Return the tree to the state it had immediately before `turn` began,
    /// undoing `turn` and every turn after it.
    ///
    /// Every entry resolves against the workspace root and is refused if it
    /// lands outside. This is the trust boundary, and it is why restore does
    /// not shell out to `cp`.
    pub fn restore(&mut self, turn: u64) -> Result<RestoreReport> {
        let mut plan: BTreeMap<String, Option<String>> = BTreeMap::new();
        for manifest in self.turns.iter().rev() {
            if manifest.turn < turn {
                break;
            }
            for (key, blob) in &manifest.entries {
                // newest first, so the oldest pre-image wins the key
                plan.insert(key.clone(), blob.clone());
            }
        }

        let mut report = RestoreReport::default();
        for (key, blob) in plan {
            match self.restore_entry(&key, blob.as_deref()) {
                Ok(Restored::Wrote(path)) => report.restored.push(path),
                Ok(Restored::Deleted(path)) => report.deleted.push(path),
                Ok(Restored::Nothing) => {}
                Err(reason) => report.refused.push(Refusal { entry: key, reason }),
            }
        }

        self.forget_from(turn)?;
        Ok(report)
    }

    fn restore_entry(
        &self,
        key: &str,
        blob: Option<&str>,
    ) -> std::result::Result<Restored, String> {
        let target = self.resolve_entry(key)?;
        match blob {
            Some(name) => {
                if !is_blob_name(name) {
                    return Err(format!("malformed blob id: {name}"));
                }
                let source = self.blobs_dir().join(name);
                let bytes = std::fs::read(&source)
                    .map_err(|err| format!("blob {name} unreadable: {err}"))?;
                if let Some(parent) = target.parent()
                    && !parent.exists()
                {
                    std::fs::create_dir_all(parent)
                        .map_err(|err| format!("cannot recreate {}: {err}", parent.display()))?;
                }
                std::fs::write(&target, &bytes)
                    .map_err(|err| format!("cannot write {}: {err}", target.display()))?;
                Ok(Restored::Wrote(target))
            }
            None => match std::fs::symlink_metadata(&target) {
                Ok(meta) if meta.is_file() => {
                    std::fs::remove_file(&target)
                        .map_err(|err| format!("cannot delete {}: {err}", target.display()))?;
                    Ok(Restored::Deleted(target))
                }
                Ok(_) => Err(format!(
                    "refusing to delete non-regular file {}",
                    target.display()
                )),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Restored::Nothing),
                Err(err) => Err(format!("cannot stat {}: {err}", target.display())),
            },
        }
    }

    /// The write-side scope check. A manifest entry that resolves outside the
    /// workspace root is refused, never restored.
    fn resolve_entry(&self, key: &str) -> std::result::Result<PathBuf, String> {
        let raw = PathBuf::from(key);
        if relative_key(&raw).as_deref() != Some(key) {
            return Err(format!(
                "manifest entry is not a workspace-relative path: {key}"
            ));
        }
        let candidate = self.workspace.join(&raw);
        crate::tools::enforce_cwd_scope(&candidate, &self.workspace, "restore")
            .map_err(|err| err.to_string())
    }

    /// Drop `turn` and everything after it: those pre-images no longer describe
    /// the tree once they have been applied.
    fn forget_from(&mut self, turn: u64) -> Result<()> {
        let dropped: Vec<u64> = self
            .turns
            .iter()
            .filter(|manifest| manifest.turn >= turn)
            .map(|manifest| manifest.turn)
            .collect();
        if dropped.is_empty() {
            return Ok(());
        }
        self.turns.retain(|manifest| manifest.turn < turn);
        self.active = None;
        for number in dropped {
            self.remove_within_store(&self.manifest_path(number))?;
        }
        self.prune_blobs()
    }

    /// Retention, on turn count and total bytes, whichever binds first.
    pub fn gc(&mut self) -> Result<()> {
        while self.turns.len() > self.retention.max_turns {
            self.drop_oldest_turn()?;
        }
        self.prune_blobs()?;
        while self.blob_bytes()? > self.retention.max_bytes && !self.turns.is_empty() {
            self.drop_oldest_turn()?;
            self.prune_blobs()?;
        }
        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        self.active = None;
        self.gc()
    }

    fn drop_oldest_turn(&mut self) -> Result<()> {
        if self.turns.is_empty() {
            return Ok(());
        }
        let manifest = self.turns.remove(0);
        self.active = match self.active {
            Some(0) | None => None,
            Some(index) => Some(index - 1),
        };
        self.remove_within_store(&self.manifest_path(manifest.turn))
    }

    fn referenced_blobs(&self) -> HashSet<&str> {
        self.turns
            .iter()
            .flat_map(|manifest| manifest.entries.values())
            .filter_map(|blob| blob.as_deref())
            .collect()
    }

    fn prune_blobs(&self) -> Result<()> {
        let referenced = self.referenced_blobs();
        let entries = match std::fs::read_dir(self.blobs_dir()) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(io_err(err)),
        };
        for entry in entries {
            let entry = entry.map_err(io_err)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if referenced.contains(name.as_str()) {
                continue;
            }
            self.remove_within_store(&entry.path())?;
        }
        Ok(())
    }

    pub fn blob_count(&self) -> Result<usize> {
        let entries = match std::fs::read_dir(self.blobs_dir()) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(io_err(err)),
        };
        let mut count = 0;
        for entry in entries {
            let entry = entry.map_err(io_err)?;
            if is_blob_name(&entry.file_name().to_string_lossy()) {
                count += 1;
            }
        }
        Ok(count)
    }

    fn blob_bytes(&self) -> Result<u64> {
        let entries = match std::fs::read_dir(self.blobs_dir()) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(io_err(err)),
        };
        let mut total = 0u64;
        for entry in entries {
            let entry = entry.map_err(io_err)?;
            if let Ok(meta) = entry.metadata() {
                total = total.saturating_add(meta.len());
            }
        }
        Ok(total)
    }

    /// The store's delete path. Nothing outside the store root is ever removed.
    fn remove_within_store(&self, path: &Path) -> Result<()> {
        let root = safe_canonicalize(&self.root);
        let canonical = safe_canonicalize(path);
        if !canonical.starts_with(&root) {
            return Err(Error::validation(format!(
                "rewind: refusing to delete outside the store root ({} is not under {})",
                canonical.display(),
                root.display()
            )));
        }
        match std::fs::remove_file(&canonical) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(io_err(err)),
        }
    }
}

enum Restored {
    Wrote(PathBuf),
    Deleted(PathBuf),
    Nothing,
}

// ============================================================================
// Process-wide hook
// ============================================================================

fn slot() -> MutexGuard<'static, Option<RewindStore>> {
    static SLOT: OnceLock<Mutex<Option<RewindStore>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

pub fn install(store: RewindStore) {
    *slot() = Some(store);
}

pub fn uninstall() -> Option<RewindStore> {
    slot().take()
}

pub fn is_active() -> bool {
    slot().is_some()
}

pub fn begin_turn(message: &str) -> Option<u64> {
    slot()
        .as_mut()
        .and_then(|store| store.begin_turn(message).ok())
}

pub fn note_bash() {
    if let Some(store) = slot().as_mut() {
        let _ = store.note_bash();
    }
}

pub fn summaries() -> Vec<TurnSummary> {
    slot()
        .as_ref()
        .map(RewindStore::summaries)
        .unwrap_or_default()
}

pub fn restore(turn: u64) -> Option<Result<RestoreReport>> {
    slot().as_mut().map(|store| store.restore(turn))
}

pub fn with_store<R>(f: impl FnOnce(&mut RewindStore) -> R) -> Option<R> {
    slot().as_mut().map(f)
}

/// Called from the single atomic-replace choke point every file tool goes
/// through. A no-op when no store is installed.
pub(crate) fn capture_pre_image(path: &Path) -> std::io::Result<()> {
    let mut guard = slot();
    let Some(store) = guard.as_mut() else {
        return Ok(());
    };
    store
        .capture(path)
        .map(|_| ())
        .map_err(|err| std::io::Error::other(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn hook_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    struct Fixture {
        _store_dir: tempfile::TempDir,
        workspace: tempfile::TempDir,
        store: Option<RewindStore>,
    }

    fn fixture(retention: Retention) -> Fixture {
        let store_dir = tempfile::tempdir().expect("store dir");
        let workspace = tempfile::tempdir().expect("workspace dir");
        let store = RewindStore::open_at(
            store_dir.path().join("session"),
            workspace.path(),
            retention,
        )
        .expect("open store");
        Fixture {
            _store_dir: store_dir,
            workspace,
            store: Some(store),
        }
    }

    impl Fixture {
        fn store(&mut self) -> &mut RewindStore {
            self.store.as_mut().expect("store")
        }

        fn path(&self, name: &str) -> PathBuf {
            self.workspace.path().join(name)
        }
    }

    /// Acceptance 1: edit a file inside a turn, restore the turn, diff against
    /// the original copy.
    #[test]
    fn restore_returns_an_edited_file_to_its_pre_turn_bytes() {
        let mut fx = fixture(Retention::default());
        let target = fx.path("note.txt");
        let original = fx.path("note.original");
        std::fs::write(&target, "line one\nline two\n").expect("seed");
        std::fs::copy(&target, &original).expect("copy original");

        fx.store().begin_turn("edit the note").expect("begin");
        fx.store().capture(&target).expect("capture");
        std::fs::write(&target, "line one\nCLOBBERED\n").expect("mutate");

        let report = fx.store().restore(1).expect("restore");
        assert_eq!(report.restored.len(), 1);
        assert!(report.refused.is_empty());

        let diff = Command::new("diff")
            .arg(&original)
            .arg(&target)
            .output()
            .expect("run diff");
        println!(
            "[acceptance 1] diff exit={:?} stdout={:?}",
            diff.status.code(),
            String::from_utf8_lossy(&diff.stdout)
        );
        assert!(diff.status.success());
        assert!(diff.stdout.is_empty());
    }

    /// Acceptance 2: a file the turn creates has no pre-image, so restore
    /// deletes it.
    #[test]
    fn restore_deletes_a_file_the_turn_created() {
        let mut fx = fixture(Retention::default());
        let target = fx.path("fresh.txt");

        fx.store().begin_turn("create a file").expect("begin");
        fx.store().capture(&target).expect("capture");
        std::fs::write(&target, "brand new\n").expect("create");
        assert!(target.exists());
        let dir = fx.workspace.path().to_path_buf();
        let ls = move |dir: &Path| {
            let out = Command::new("ls").arg(dir).output().expect("run ls");
            String::from_utf8_lossy(&out.stdout).replace('\n', " ")
        };
        let before = ls(&dir);

        let report = fx.store().restore(1).expect("restore");
        assert_eq!(report.deleted.len(), 1);

        println!(
            "[acceptance 2] ls before = {:?}, ls after = {:?}",
            before.trim(),
            ls(&dir).trim()
        );
        assert!(!target.exists());
    }

    /// Acceptance 3: three turns on one path cost three blobs, and restoring to
    /// turn 1 yields the bytes turn 1 started from.
    #[test]
    fn three_turns_on_one_path_cost_three_blobs_and_rewind_to_the_first() {
        let mut fx = fixture(Retention::default());
        let target = fx.path("serial.txt");
        std::fs::write(&target, "v0\n").expect("seed");

        for version in 1..=3 {
            fx.store()
                .begin_turn(&format!("turn {version}"))
                .expect("begin");
            fx.store().capture(&target).expect("capture");
            std::fs::write(&target, format!("v{version}\n")).expect("mutate");
        }

        let blobs = fx.store().blob_count().expect("blob count");
        println!("[acceptance 3] blob count after 3 turns = {blobs}");
        assert_eq!(blobs, 3);

        fx.store().restore(1).expect("restore");
        let content = std::fs::read_to_string(&target).expect("read back");
        println!("[acceptance 3] content after restore(1) = {content:?}");
        assert_eq!(content, "v0\n");
    }

    /// Acceptance 4: a hand-edited manifest pointing outside the workspace is
    /// refused, not restored.
    #[test]
    fn a_manifest_entry_outside_the_workspace_is_refused() {
        let mut fx = fixture(Retention::default());
        let target = fx.path("in-scope.txt");
        std::fs::write(&target, "safe\n").expect("seed");
        fx.store().begin_turn("tampered turn").expect("begin");
        fx.store().capture(&target).expect("capture");

        let manifest_path = fx.store().manifest_path(1);
        let raw = std::fs::read(&manifest_path).expect("read manifest");
        let mut manifest: TurnManifest = serde_json::from_slice(&raw).expect("parse manifest");
        let blob = manifest
            .entries
            .values()
            .next()
            .cloned()
            .expect("captured blob");
        manifest
            .entries
            .insert("../../../../../../etc/passwd".to_string(), blob.clone());
        manifest
            .entries
            .insert("/etc/passwd".to_string(), blob.clone());
        // a plain-looking key whose in-workspace symlink resolves out: only the
        // scope check catches this one
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/passwd", fx.path("escape.txt"))
                .expect("symlink escape");
            manifest.entries.insert("escape.txt".to_string(), blob);
        }
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("serialize"),
        )
        .expect("write tampered manifest");

        let before = std::fs::metadata("/etc/passwd")
            .ok()
            .and_then(|m| m.modified().ok());

        let mut reopened = RewindStore::open_at(
            fx.store().root().to_path_buf(),
            fx.workspace.path(),
            Retention::default(),
        )
        .expect("reopen");
        let report = reopened.restore(1).expect("restore");
        for refusal in &report.refused {
            println!(
                "[acceptance 4] refused {:?}: {}",
                refusal.entry, refusal.reason
            );
        }
        let after = std::fs::metadata("/etc/passwd")
            .ok()
            .and_then(|m| m.modified().ok());
        println!("[acceptance 4] /etc/passwd mtime before={before:?} after={after:?}");

        let expected_refusals = if cfg!(unix) { 3 } else { 2 };
        assert_eq!(report.refused.len(), expected_refusals);
        assert!(
            report
                .restored
                .iter()
                .all(|path| path.starts_with(fx.workspace.path()))
        );
        assert_eq!(before, after);
    }

    /// Acceptance 5: the manifest records that the turn ran bash.
    #[test]
    fn a_turn_that_ran_bash_says_so_in_its_manifest() {
        let mut fx = fixture(Retention::default());
        fx.store().begin_turn("run a command").expect("begin");
        fx.store().note_bash().expect("note bash");

        let manifest_path = fx.store().manifest_path(1);
        let raw = std::fs::read_to_string(&manifest_path).expect("read manifest");
        println!("[acceptance 5] manifest = {raw}");
        assert!(raw.contains("\"ran_bash\": true"));
        assert!(fx.store().summaries()[0].ran_bash);
    }

    /// Acceptance 6: retention capped at two turns keeps two, and the blobs the
    /// dropped turns held are gone.
    #[test]
    fn retention_keeps_the_cap_and_drops_the_older_blobs() {
        let mut fx = fixture(Retention {
            max_turns: 2,
            max_bytes: DEFAULT_MAX_BYTES,
        });
        let target = fx.path("rolling.txt");
        std::fs::write(&target, "v0\n").expect("seed");

        for version in 1..=5 {
            fx.store()
                .begin_turn(&format!("turn {version}"))
                .expect("begin");
            fx.store().capture(&target).expect("capture");
            std::fs::write(&target, format!("v{version}\n")).expect("mutate");
            fx.store().gc().expect("gc");
        }

        let kept: Vec<u64> = fx.store().turns().iter().map(|turn| turn.turn).collect();
        let blobs = fx.store().blob_count().expect("blob count");
        println!("[acceptance 6] kept turns = {kept:?}, blobs = {blobs}");
        assert_eq!(kept, vec![4, 5]);
        assert_eq!(blobs, 2);
    }

    #[test]
    fn gc_also_runs_on_open_because_sessions_do_not_always_close_cleanly() {
        let mut fx = fixture(Retention {
            max_turns: 2,
            max_bytes: DEFAULT_MAX_BYTES,
        });
        let target = fx.path("dirty.txt");
        std::fs::write(&target, "v0\n").expect("seed");
        for version in 1..=4 {
            fx.store().begin_turn("turn").expect("begin");
            fx.store().capture(&target).expect("capture");
            std::fs::write(&target, format!("v{version}\n")).expect("mutate");
        }
        let root = fx.store().root().to_path_buf();
        assert_eq!(fx.store().turns().len(), 4);

        let reopened = RewindStore::open_at(
            root,
            fx.workspace.path(),
            Retention {
                max_turns: 2,
                max_bytes: DEFAULT_MAX_BYTES,
            },
        )
        .expect("reopen");
        assert_eq!(reopened.turns().len(), 2);
        assert_eq!(reopened.blob_count().expect("blob count"), 2);
    }

    #[test]
    fn a_path_outside_the_workspace_is_not_captured() {
        let mut fx = fixture(Retention::default());
        let outside = tempfile::tempdir().expect("outside");
        let path = outside.path().join("elsewhere.txt");
        std::fs::write(&path, "not ours\n").expect("seed");
        fx.store().begin_turn("turn").expect("begin");
        assert!(!fx.store().capture(&path).expect("capture"));
        assert_eq!(fx.store().blob_count().expect("blob count"), 0);
    }

    #[test]
    fn a_second_write_to_the_same_path_in_a_turn_is_free() {
        let mut fx = fixture(Retention::default());
        let target = fx.path("twice.txt");
        std::fs::write(&target, "v0\n").expect("seed");
        fx.store().begin_turn("turn").expect("begin");
        assert!(fx.store().capture(&target).expect("first"));
        std::fs::write(&target, "v1\n").expect("mutate");
        assert!(!fx.store().capture(&target).expect("second"));
        assert_eq!(fx.store().blob_count().expect("blob count"), 1);
    }

    #[test]
    fn gc_refuses_to_delete_outside_the_store_root() {
        let mut fx = fixture(Retention::default());
        let victim = fx.path("not-a-blob.txt");
        std::fs::write(&victim, "keep me\n").expect("seed");
        let err = fx
            .store()
            .remove_within_store(&victim)
            .expect_err("must refuse");
        println!("[guard] {err}");
        assert!(victim.exists());
    }

    /// The hook, exercised through the real `write` and `edit` tools rather
    /// than by calling `capture` directly.
    #[test]
    fn the_write_and_edit_tools_record_pre_images_through_the_hook() {
        let _guard = hook_lock();
        let store_dir = tempfile::tempdir().expect("store dir");
        let workspace = tempfile::tempdir().expect("workspace");
        let workspace_root = safe_canonicalize(workspace.path());
        let existing = workspace_root.join("existing.txt");
        std::fs::write(&existing, "before\n").expect("seed");
        let created = workspace_root.join("created.txt");

        let mut store = RewindStore::open_at(
            store_dir.path().join("session"),
            &workspace_root,
            Retention::default(),
        )
        .expect("open store");
        store.begin_turn("agent turn").expect("begin");
        install(store);

        asupersync::test_utils::run_test(|| {
            let workspace_root = workspace_root.clone();
            let existing = existing.clone();
            let created = created.clone();
            async move {
                let write = crate::tools::WriteTool::new(&workspace_root);
                crate::tools::Tool::execute(
                    &write,
                    "call-1",
                    serde_json::json!({
                        "path": created.to_string_lossy(),
                        "content": "new file\n",
                    }),
                    None,
                )
                .await
                .expect("write tool");

                let edit = crate::tools::EditTool::new(&workspace_root);
                crate::tools::Tool::execute(
                    &edit,
                    "call-2",
                    serde_json::json!({
                        "path": existing.to_string_lossy(),
                        "oldText": "before\n",
                        "newText": "after\n",
                    }),
                    None,
                )
                .await
                .expect("edit tool");

                let bash = crate::tools::BashTool::new(&workspace_root);
                crate::tools::Tool::execute(
                    &bash,
                    "call-3",
                    serde_json::json!({ "command": "echo rewind-cannot-undo-me" }),
                    None,
                )
                .await
                .expect("bash tool");
            }
        });

        let mut store = uninstall().expect("store still installed");
        // store is process-global, so a concurrent agent test can open a turn
        // underneath this one and split the entries. the hook is what is under test
        let (entries, ran_bash) = store.turns().iter().fold(
            (BTreeMap::new(), false),
            |(mut entries, ran_bash), manifest| {
                entries.extend(manifest.entries.clone());
                (entries, ran_bash | manifest.ran_bash)
            },
        );
        println!(
            "[hook] entries = {:?}, ran_bash = {ran_bash}",
            entries.keys().collect::<Vec<_>>()
        );
        assert_eq!(entries.get("created.txt"), Some(&None));
        assert!(entries.get("existing.txt").expect("entry").is_some());
        assert!(ran_bash);

        let report = store.restore(1).expect("restore");
        assert!(report.refused.is_empty());
        assert!(!created.exists());
        assert_eq!(
            std::fs::read_to_string(&existing).expect("read back"),
            "before\n"
        );
    }
}
