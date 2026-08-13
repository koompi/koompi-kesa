//! Every environment variable this crate reads is `KESA_<SUFFIX>`.
//!
//! Callers pass the suffix, never the whole name, so the current name and the
//! legacy `KODE_` name it falls back to cannot drift apart.

use std::collections::HashSet;
use std::ffi::OsString;
use std::sync::{LazyLock, Mutex, PoisonError};

const PREFIX: &str = "KESA_";
const LEGACY_PREFIX: &str = "KODE_";

/// Suffixes a user sets by hand on their own install, and the only ones that
/// still answer to `KODE_`. Everything else in this crate's environment surface
/// is set by the repository's own harnesses, so it was renamed outright.
///
/// Deleting this list is the whole of the deprecation removal.
const USER_FACING: &[&str] = &[
    "CLEAR_ON_SHRINK",
    "CODING_AGENT_DIR",
    "CONFIG_PATH",
    "EXTENSION_ALLOW_DANGEROUS",
    "EXTENSION_INDEX_PATH",
    "EXTENSION_POLICY",
    "HARDWARE_CURSOR",
    "HIDE_CWD_IN_PROMPT",
    "HTTP_REQUEST_TIMEOUT_SECS",
    "MODEL",
    "MODELS_OVERRIDE",
    "NO_MOUSE_CAPTURE",
    "PACKAGE_DIR",
    "PERMISSION_MODE",
    "PROVIDER",
    "REPAIR_POLICY",
    "SESSIONS_DIR",
    "SHARE_VIEWER_URL",
    "WEB_ALLOW_PRIVATE_HOSTS",
    "WEB_SEARCH_API_KEY",
    "WEB_SEARCH_PROVIDER",
];

static WARNED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// The current name for `suffix`, for messages and for spawning children.
#[must_use]
pub fn name(suffix: &str) -> String {
    format!("{PREFIX}{suffix}")
}

/// The pre-rename name for `suffix`.
#[must_use]
pub fn legacy_name(suffix: &str) -> String {
    format!("{LEGACY_PREFIX}{suffix}")
}

#[must_use]
pub fn var(suffix: &str) -> Option<String> {
    std::env::var(name(suffix))
        .ok()
        .or_else(|| legacy(suffix).and_then(|value| value.into_string().ok()))
}

#[must_use]
pub fn var_os(suffix: &str) -> Option<OsString> {
    std::env::var_os(name(suffix)).or_else(|| legacy(suffix))
}

#[must_use]
pub fn is_set(suffix: &str) -> bool {
    var_os(suffix).is_some()
}

/// The legacy value for `suffix`, for readers that already consulted the
/// current name themselves. clap's `env =` attribute is the only one.
#[must_use]
pub fn legacy_fallback(suffix: &str) -> Option<String> {
    if std::env::var_os(name(suffix)).is_some() {
        return None;
    }
    var(suffix)
}

fn legacy(suffix: &str) -> Option<OsString> {
    if !USER_FACING.contains(&suffix) {
        return None;
    }
    let value = std::env::var_os(legacy_name(suffix))?;
    warn_once(suffix);
    Some(value)
}

fn warn_once(suffix: &str) {
    let mut warned = WARNED.lock().unwrap_or_else(PoisonError::into_inner);
    if warned.insert(suffix.to_string()) {
        eprintln!(
            "Warning: {} is deprecated and will stop being read one release after this one; rename it to {}.",
            legacy_name(suffix),
            name(suffix)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{USER_FACING, legacy_name, name};

    #[test]
    fn names_carry_the_current_prefix() {
        assert_eq!(name("MODEL"), "KESA_MODEL");
        assert_eq!(legacy_name("MODEL"), "KODE_MODEL");
    }

    #[test]
    fn user_facing_list_holds_suffixes_not_whole_names() {
        for suffix in USER_FACING {
            assert!(
                !suffix.starts_with("KESA_") && !suffix.starts_with("KODE_"),
                "{suffix} is a whole name, not a suffix"
            );
        }
    }

    #[test]
    fn user_facing_list_is_sorted_and_unique() {
        let mut sorted = USER_FACING.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, USER_FACING);
    }
}
