//! Shell hooks on the tool-call boundary.
//!
//! A hook is a shell command the user configured in their own settings. It runs
//! with the user's privileges, unsandboxed, exactly like a command they typed:
//! there is no jail here by design.
//!
//! The exit code is the contract:
//!
//! - `0` allows the call.
//! - `2` blocks it, and the hook's stderr is what the model sees as the tool result.
//! - anything else is logged as a warning and allows the call.
//!
//! A hook that outlives its timeout is killed and treated as a warning.
//!
//! ## What reaches `SessionEnd`
//!
//! [`SessionLifecycle`] fires it from three places: the guard's `Drop` on any
//! exit that unwinds or returns, a chained panic hook, and a `SIGINT` handler
//! when nothing else in the process already owns `SIGINT`. It fires once per
//! session whichever arrives first. The cases it still misses are listed on
//! [`SessionLifecycle`] and in `docs/hooks.md`.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Seconds a hook may run before it is killed, when the entry sets no timeout.
pub const DEFAULT_TIMEOUT_SECS: u64 = 60;

/// Seconds a session lifecycle hook may run before it is killed, when the entry
/// sets no timeout. Shorter than [`DEFAULT_TIMEOUT_SECS`] because these run on
/// the startup and exit paths, where a stalled hook reads as a hung program.
pub const LIFECYCLE_TIMEOUT_SECS: u64 = 5;

/// Exit status used when the `SIGINT` handler this module installed is the one
/// that ends the process. Matches what a shell reports for a default `SIGINT`.
const INTERRUPT_EXIT_CODE: i32 = 130;

/// Exit code that blocks the call and feeds stderr back to the model.
const BLOCK_EXIT_CODE: i32 = 2;

const POLL_INTERVAL: Duration = Duration::from_millis(5);
const STDERR_GRACE: Duration = Duration::from_secs(2);
const MAX_CAPTURE_BYTES: usize = 16 * 1024;

/// Points in the turn where a hook can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    /// After the policy decision, before the tool spawns.
    PreToolUse,
    /// After the tool produced its result.
    PostToolUse,
    /// When the user submits a prompt.
    UserPromptSubmit,
    /// When the agent finishes a turn.
    Stop,
    /// When a session is created, before it reads any input.
    SessionStart,
    /// When a session ends. See [`SessionLifecycle`] for what reaches it.
    SessionEnd,
    /// Before compaction discards anything. Afterwards the history the hook
    /// would want to look at is already gone.
    PreCompact,
    /// When a subagent finishes a turn, in the subagent's own process.
    SubagentStop,
    /// When KESA has something to tell the user, including turn-end alerts.
    Notification,
    /// When the user aborts a turn before it finishes.
    Interrupt,
}

impl HookEvent {
    /// Every event, so a new variant cannot be forgotten by the code that has
    /// to walk all of them.
    pub const ALL: [Self; 10] = [
        Self::PreToolUse,
        Self::PostToolUse,
        Self::UserPromptSubmit,
        Self::Stop,
        Self::SessionStart,
        Self::SessionEnd,
        Self::PreCompact,
        Self::SubagentStop,
        Self::Notification,
        Self::Interrupt,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::Stop => "Stop",
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::PreCompact => "PreCompact",
            Self::SubagentStop => "SubagentStop",
            Self::Notification => "Notification",
            Self::Interrupt => "Interrupt",
        }
    }
}

/// One configured hook.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HookEntry {
    /// Tool-name pattern. `*` or absent matches every tool, `|` separates
    /// alternatives, and matching is case-insensitive.
    pub matcher: Option<String>,
    /// Shell command run with `/bin/sh -c`.
    pub command: Option<String>,
    /// Seconds before the hook is killed. Defaults to [`DEFAULT_TIMEOUT_SECS`].
    #[serde(alias = "timeoutSeconds", alias = "timeout")]
    pub timeout_seconds: Option<u64>,
}

/// The `hooks` settings section, keyed by event.
///
/// ```json
/// {
///   "hooks": {
///     "PreToolUse": [
///       { "matcher": "bash", "command": "~/.kesa/deny-rm.sh", "timeout": 5 }
///     ]
///   }
/// }
/// ```
///
/// Every command here runs with the user's own privileges and is not sandboxed,
/// so only a settings file the user controls may supply one. Hooks from a
/// project's `.kesa/settings.json` are ignored unless the *global* settings file
/// sets `trustProjectHooks`, because a cloned repository can carry that file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HooksConfig {
    #[serde(rename = "PreToolUse", alias = "preToolUse", alias = "pre_tool_use")]
    pub pre_tool_use: Option<Vec<HookEntry>>,
    #[serde(rename = "PostToolUse", alias = "postToolUse", alias = "post_tool_use")]
    pub post_tool_use: Option<Vec<HookEntry>>,
    #[serde(
        rename = "UserPromptSubmit",
        alias = "userPromptSubmit",
        alias = "user_prompt_submit"
    )]
    pub user_prompt_submit: Option<Vec<HookEntry>>,
    #[serde(rename = "Stop", alias = "stop")]
    pub stop: Option<Vec<HookEntry>>,
    #[serde(
        rename = "SessionStart",
        alias = "sessionStart",
        alias = "session_start"
    )]
    pub session_start: Option<Vec<HookEntry>>,
    #[serde(rename = "SessionEnd", alias = "sessionEnd", alias = "session_end")]
    pub session_end: Option<Vec<HookEntry>>,
    #[serde(rename = "PreCompact", alias = "preCompact", alias = "pre_compact")]
    pub pre_compact: Option<Vec<HookEntry>>,
    #[serde(
        rename = "SubagentStop",
        alias = "subagentStop",
        alias = "subagent_stop"
    )]
    pub subagent_stop: Option<Vec<HookEntry>>,
    #[serde(rename = "Notification", alias = "notification")]
    pub notification: Option<Vec<HookEntry>>,
    #[serde(rename = "Interrupt", alias = "interrupt")]
    pub interrupt: Option<Vec<HookEntry>>,
    /// Opt in to running hooks from the project settings file. Honored only
    /// when it is set in the global settings file.
    #[serde(alias = "trustProjectHooks")]
    pub trust_project_hooks: Option<bool>,
}

impl HooksConfig {
    #[must_use]
    pub fn entries(&self, event: HookEvent) -> &[HookEntry] {
        let entries = match event {
            HookEvent::PreToolUse => &self.pre_tool_use,
            HookEvent::PostToolUse => &self.post_tool_use,
            HookEvent::UserPromptSubmit => &self.user_prompt_submit,
            HookEvent::Stop => &self.stop,
            HookEvent::SessionStart => &self.session_start,
            HookEvent::SessionEnd => &self.session_end,
            HookEvent::PreCompact => &self.pre_compact,
            HookEvent::SubagentStop => &self.subagent_stop,
            HookEvent::Notification => &self.notification,
            HookEvent::Interrupt => &self.interrupt,
        };
        entries.as_deref().unwrap_or(&[])
    }

    #[must_use]
    pub fn has_hooks(&self) -> bool {
        HookEvent::ALL
            .iter()
            .any(|event| !self.entries(*event).is_empty())
    }

    /// Concatenates hook lists per event: a settings file that adds one hook
    /// must not drop the hooks the other file installed.
    ///
    /// It lives here rather than beside the rest of the settings merge so that
    /// adding an event is one file's work and cannot silently lose a list.
    #[must_use]
    pub fn merged(base: Self, other: Self) -> Self {
        Self {
            pre_tool_use: concat_entries(base.pre_tool_use, other.pre_tool_use),
            post_tool_use: concat_entries(base.post_tool_use, other.post_tool_use),
            user_prompt_submit: concat_entries(base.user_prompt_submit, other.user_prompt_submit),
            stop: concat_entries(base.stop, other.stop),
            session_start: concat_entries(base.session_start, other.session_start),
            session_end: concat_entries(base.session_end, other.session_end),
            pre_compact: concat_entries(base.pre_compact, other.pre_compact),
            subagent_stop: concat_entries(base.subagent_stop, other.subagent_stop),
            notification: concat_entries(base.notification, other.notification),
            interrupt: concat_entries(base.interrupt, other.interrupt),
            trust_project_hooks: other.trust_project_hooks.or(base.trust_project_hooks),
        }
    }
}

fn concat_entries(
    base: Option<Vec<HookEntry>>,
    other: Option<Vec<HookEntry>>,
) -> Option<Vec<HookEntry>> {
    match (base, other) {
        (Some(base), Some(other)) => {
            let mut merged = base;
            merged.extend(other);
            Some(merged)
        }
        (None, other) => other,
        (base, None) => base,
    }
}

/// What the caller does with the call the hooks just saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
    Allow,
    Block { reason: String },
}

/// Runs the configured hooks.
#[derive(Debug, Clone, Default)]
pub struct HookRunner {
    config: HooksConfig,
}

impl HookRunner {
    #[must_use]
    pub const fn new(config: HooksConfig) -> Self {
        Self { config }
    }

    /// Load hooks from the user's settings, falling back to none.
    #[must_use]
    pub fn from_settings() -> Self {
        let config = crate::config::Config::load().unwrap_or_else(|err| {
            tracing::warn!("hooks: settings unreadable, running without hooks: {err}");
            crate::config::Config::default()
        });
        Self::new(config.hooks.unwrap_or_default())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.config.has_hooks()
    }

    /// Run the `PreToolUse` hooks matching `tool_name`.
    pub async fn pre_tool_use(&self, tool_name: &str, tool_input: &Value) -> HookDecision {
        self.dispatch(HookEvent::PreToolUse, tool_name, || {
            json!({
                "hook_event_name": HookEvent::PreToolUse.as_str(),
                "tool_name": tool_name,
                "tool_input": tool_input,
            })
        })
        .await
    }

    /// Run the `PostToolUse` hooks matching `tool_name`.
    ///
    /// The tool has already run, so a block here cannot stop it: the reason
    /// comes back as feedback appended to the result.
    pub async fn post_tool_use(
        &self,
        tool_name: &str,
        tool_input: &Value,
        tool_response: &Value,
    ) -> HookDecision {
        self.dispatch(HookEvent::PostToolUse, tool_name, || {
            json!({
                "hook_event_name": HookEvent::PostToolUse.as_str(),
                "tool_name": tool_name,
                "tool_input": tool_input,
                "tool_response": tool_response,
            })
        })
        .await
    }

    /// Run the `UserPromptSubmit` hooks. A block rejects the prompt.
    pub async fn user_prompt_submit(&self, prompt: &str) -> HookDecision {
        self.dispatch(HookEvent::UserPromptSubmit, "", || {
            json!({
                "hook_event_name": HookEvent::UserPromptSubmit.as_str(),
                "prompt": prompt,
            })
        })
        .await
    }

    /// Run the `Stop` hooks. A block asks the caller to keep the turn going.
    ///
    /// `stop_hook_active` is true when this turn only exists because a `Stop`
    /// hook blocked the last one; a hook that ignores it blocks forever.
    pub async fn stop(&self, stop_hook_active: bool) -> HookDecision {
        self.dispatch(HookEvent::Stop, "", || {
            json!({
                "hook_event_name": HookEvent::Stop.as_str(),
                "stop_hook_active": stop_hook_active,
            })
        })
        .await
    }

    /// Run the `SubagentStop` hooks. Fires in the subagent's own process, where
    /// a subagent is a child `kesa` rather than a task inside this one.
    pub async fn subagent_stop(&self, stop_hook_active: bool) -> HookDecision {
        self.dispatch(HookEvent::SubagentStop, "", || {
            json!({
                "hook_event_name": HookEvent::SubagentStop.as_str(),
                "stop_hook_active": stop_hook_active,
            })
        })
        .await
    }

    /// Run the `PreCompact` hooks, before compaction discards anything.
    ///
    /// `trigger` is `auto` or `manual`. A block cancels nothing: compaction is
    /// already committed by the time the caller has a preparation to describe,
    /// so the reason is logged and the caller proceeds.
    pub async fn pre_compact(
        &self,
        trigger: &str,
        custom_instructions: Option<&str>,
    ) -> HookDecision {
        self.dispatch(HookEvent::PreCompact, "", || {
            json!({
                "hook_event_name": HookEvent::PreCompact.as_str(),
                "trigger": trigger,
                "custom_instructions": custom_instructions,
            })
        })
        .await
    }

    /// Run the `Notification` hooks. This is the path a user's `notify-send`
    /// takes, so KESA never grows a desktop dependency of its own.
    pub async fn notification(&self, message: &str) -> HookDecision {
        self.dispatch(HookEvent::Notification, "", || {
            json!({
                "hook_event_name": HookEvent::Notification.as_str(),
                "message": message,
            })
        })
        .await
    }

    /// Run the `Interrupt` hooks, after the user aborted a turn. `Stop` does
    /// not fire on an interrupt; this does. There is nothing left to block, so
    /// the caller ignores the decision.
    pub async fn interrupt(&self, session_id: &str) -> HookDecision {
        self.dispatch(HookEvent::Interrupt, "", || {
            json!({
                "hook_event_name": HookEvent::Interrupt.as_str(),
                "session_id": session_id,
            })
        })
        .await
    }

    /// Run the `SessionStart` hooks. `source` is `startup` or `resume`.
    pub async fn session_start(&self, session_id: &str, source: &str) -> HookDecision {
        self.dispatch(HookEvent::SessionStart, "", || {
            json!({
                "hook_event_name": HookEvent::SessionStart.as_str(),
                "session_id": session_id,
                "source": source,
            })
        })
        .await
    }

    /// Run the `SessionEnd` hooks. Blocking, because the callers are `Drop`, a
    /// panic hook and a signal handler thread, none of which can await.
    pub fn session_end_blocking(&self, session_id: &str, reason: SessionEndReason) {
        let _ = self.dispatch_blocking(
            HookEvent::SessionEnd,
            json!({
                "hook_event_name": HookEvent::SessionEnd.as_str(),
                "session_id": session_id,
                "reason": reason.as_str(),
            }),
        );
    }

    fn matched(&self, event: HookEvent, matcher_key: &str) -> Vec<HookEntry> {
        self.config
            .entries(event)
            .iter()
            .filter(|entry| {
                entry
                    .command
                    .as_deref()
                    .is_some_and(|c| !c.trim().is_empty())
                    && matcher_matches(entry.matcher.as_deref().unwrap_or("*"), matcher_key)
            })
            .cloned()
            .collect()
    }

    async fn dispatch<F>(&self, event: HookEvent, matcher_key: &str, payload: F) -> HookDecision
    where
        F: FnOnce() -> Value,
    {
        let matched = self.matched(event, matcher_key);
        if matched.is_empty() {
            return HookDecision::Allow;
        }

        let payload = encode_payload(payload());

        for entry in matched {
            let command = entry.command.unwrap_or_default();
            let timeout = entry_timeout(entry.timeout_seconds, DEFAULT_TIMEOUT_SECS);
            let decision = run_hook(event, command, payload.clone(), timeout).await;
            if let HookDecision::Block { .. } = decision {
                return decision;
            }
        }

        HookDecision::Allow
    }

    fn dispatch_blocking(&self, event: HookEvent, payload: Value) -> HookDecision {
        let matched = self.matched(event, "");
        if matched.is_empty() {
            return HookDecision::Allow;
        }

        let payload = encode_payload(payload);

        for entry in matched {
            let command = entry.command.unwrap_or_default();
            let timeout = entry_timeout(entry.timeout_seconds, LIFECYCLE_TIMEOUT_SECS);
            let run = run_hook_blocking(&command, &payload, timeout);
            let decision = interpret(event, &command, timeout, run);
            if let HookDecision::Block { .. } = decision {
                return decision;
            }
        }

        HookDecision::Allow
    }
}

fn entry_timeout(configured: Option<u64>, default_secs: u64) -> Duration {
    Duration::from_secs(configured.unwrap_or(default_secs).max(1))
}

fn encode_payload(payload: Value) -> String {
    serde_json::to_string(&add_cwd(payload)).unwrap_or_else(|_| "{}".to_string())
}

/// What ended the session, as reported to a `SessionEnd` hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEndReason {
    /// The session was dropped: a clean exit, or an error that unwound to one.
    Exit,
    /// `SIGINT` reached the handler this module installed.
    Interrupt,
    /// A panic is unwinding. The panic itself is untouched.
    Panic,
}

impl SessionEndReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exit => "exit",
            Self::Interrupt => "interrupt",
            Self::Panic => "panic",
        }
    }
}

struct LifecycleState {
    runner: Arc<HookRunner>,
    session_id: String,
    ended: AtomicBool,
}

impl LifecycleState {
    fn end(&self, reason: SessionEndReason) {
        if self.ended.swap(true, Ordering::SeqCst) {
            return;
        }
        self.runner.session_end_blocking(&self.session_id, reason);
    }
}

/// Sessions whose `SessionEnd` has not fired yet, so the panic hook and the
/// signal handler can reach them without either owning the session.
static ACTIVE: Mutex<Vec<Arc<LifecycleState>>> = Mutex::new(Vec::new());

fn end_active_sessions(reason: SessionEndReason) {
    let states = {
        let active = ACTIVE.lock().unwrap_or_else(PoisonError::into_inner);
        active.clone()
    };
    for state in states {
        state.end(reason);
    }
}

/// Fires `SessionStart` when built and `SessionEnd` once, whichever of three
/// paths gets there first.
///
/// - `Drop`, which covers a clean exit and any error that unwinds to one.
/// - A panic hook, chained onto whatever hook was already installed. It runs
///   after the previous hook, so the panic message and the exit code are
///   exactly what they were without it.
/// - A `SIGINT` handler, installed by [`Self::arm_interrupt`] only when nothing
///   else in the process has claimed `SIGINT`. Where something has, that owner
///   is already turning `SIGINT` into a graceful shutdown, which unwinds to the
///   `Drop` above.
///
/// ## What it still misses
///
/// - `SIGKILL` and `SIGSTOP`, which cannot be caught at all. No amount of care
///   here changes that, and a hook contract that implied otherwise would be
///   worse than one that says so.
/// - `SIGTERM`, `SIGHUP` and `SIGQUIT`. The interrupt handler is `ctrlc`, which
///   is compiled here without its `termination` feature, so it covers `SIGINT`
///   alone.
/// - A `SIGINT` arriving before [`Self::arm_interrupt`] runs, which happens on
///   the first turn rather than at construction so that a mode which wants
///   `SIGINT` for its own graceful shutdown gets to claim it first.
/// - `std::process::exit`, `abort`, and a build with `panic = "abort"`. None of
///   them unwind, so no `Drop` runs.
/// - Power loss, and the process being killed by the OOM killer.
pub struct SessionLifecycle {
    state: Arc<LifecycleState>,
    armed: bool,
}

impl SessionLifecycle {
    /// Start watching for the end of the session. `SessionStart` is the
    /// caller's to fire: it has a runtime to await on, and this does not.
    ///
    /// Inert when no `SessionEnd` hook is configured: no panic hook is chained
    /// and no signal handler is installed, so a session with no hooks leaves
    /// the process exactly as it found it.
    #[must_use]
    pub fn watch(runner: Arc<HookRunner>, session_id: impl Into<String>) -> Self {
        let session_id = session_id.into();
        let armed = !runner.config.entries(HookEvent::SessionEnd).is_empty();

        let state = Arc::new(LifecycleState {
            runner,
            session_id,
            ended: AtomicBool::new(false),
        });

        if armed {
            install_panic_hook();
            ACTIVE
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(Arc::clone(&state));
        }

        Self { state, armed }
    }

    /// Claim `SIGINT` for `SessionEnd`, if no one else has.
    ///
    /// Idempotent, and deliberately not part of [`Self::watch`]: the RPC, ACP
    /// and print modes install their own `SIGINT` handler after the session is
    /// built, and stealing it from them would turn their graceful shutdown into
    /// a dead key.
    pub fn arm_interrupt(&self) {
        static ARMED: AtomicBool = AtomicBool::new(false);

        if !self.armed || ARMED.swap(true, Ordering::SeqCst) {
            return;
        }

        match ctrlc::try_set_handler(|| {
            end_active_sessions(SessionEndReason::Interrupt);
            std::process::exit(INTERRUPT_EXIT_CODE);
        }) {
            Ok(()) => tracing::debug!("SessionEnd hooks armed on SIGINT"),
            Err(err) => tracing::debug!(
                "SIGINT is already owned ({err}); SessionEnd rides that handler's shutdown instead"
            ),
        }
    }

    /// End the session now, without waiting for the guard to drop.
    pub fn end(&self, reason: SessionEndReason) {
        self.state.end(reason);
    }
}

impl Drop for SessionLifecycle {
    fn drop(&mut self) {
        self.state.end(SessionEndReason::Exit);
        if self.armed {
            let mut active = ACTIVE.lock().unwrap_or_else(PoisonError::into_inner);
            active.retain(|state| !Arc::ptr_eq(state, &self.state));
        }
    }
}

fn install_panic_hook() {
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Previous hook first: the panic message must not wait behind a user's
        // shell script, and losing a crash to gain a hook is a bad trade.
        previous(info);
        end_active_sessions(SessionEndReason::Panic);
    }));
}

fn add_cwd(mut payload: Value) -> Value {
    if let Some(object) = payload.as_object_mut() {
        if let Ok(cwd) = std::env::current_dir() {
            object.insert("cwd".to_string(), json!(cwd.to_string_lossy()));
        }
    }
    payload
}

/// Case-insensitive tool-name match. `*` wildcards anywhere, `|` alternation.
#[must_use]
pub fn matcher_matches(pattern: &str, name: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return true;
    }
    pattern
        .split('|')
        .any(|alternative| glob_ci(alternative.trim(), name))
}

fn glob_ci(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let pattern = pattern.to_ascii_lowercase();
    let value = value.to_ascii_lowercase();

    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == value;
    }

    let Some(rest) = value.strip_prefix(parts[0]) else {
        return false;
    };
    let mut rest = rest;
    let last = parts.len() - 1;
    for part in &parts[1..last] {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.ends_with(parts[last])
}

struct HookRun {
    code: Option<i32>,
    stderr: String,
    timed_out: bool,
}

async fn run_hook(
    event: HookEvent,
    command: String,
    payload: String,
    timeout: Duration,
) -> HookDecision {
    let logged = command.clone();
    let run =
        asupersync::runtime::spawn_blocking(move || run_hook_blocking(&command, &payload, timeout))
            .await;

    match run {
        Err(err) => {
            tracing::warn!(
                "hook `{logged}` for {} failed to start: {err}",
                event.as_str()
            );
            HookDecision::Allow
        }
        Ok(run) => interpret(event, &logged, timeout, Ok(run)),
    }
}

fn interpret(
    event: HookEvent,
    command: &str,
    timeout: Duration,
    run: std::io::Result<HookRun>,
) -> HookDecision {
    let run = match run {
        Ok(run) => run,
        Err(err) => {
            tracing::warn!(
                "hook `{command}` for {} failed to start: {err}",
                event.as_str()
            );
            return HookDecision::Allow;
        }
    };

    if run.timed_out {
        tracing::warn!(
            "hook `{command}` for {} exceeded {}s and was killed",
            event.as_str(),
            timeout.as_secs()
        );
        return HookDecision::Allow;
    }

    match run.code {
        Some(0) => HookDecision::Allow,
        Some(BLOCK_EXIT_CODE) => {
            let reason = run.stderr.trim();
            let reason = if reason.is_empty() {
                format!("blocked by a {} hook", event.as_str())
            } else {
                reason.to_string()
            };
            HookDecision::Block { reason }
        }
        code => {
            tracing::warn!(
                "hook `{command}` for {} exited with {code:?}: {}",
                event.as_str(),
                run.stderr.trim()
            );
            HookDecision::Allow
        }
    }
}

fn run_hook_blocking(command: &str, payload: &str, timeout: Duration) -> std::io::Result<HookRun> {
    let mut child = shell_command(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Each pipe gets its own thread: a hook that ignores stdin, or writes more
    // than a pipe buffer holds, must not deadlock the poll loop below.
    let mut stdin = child.stdin.take();
    let payload = payload.to_string();
    std::thread::spawn(move || {
        if let Some(mut stdin) = stdin.take() {
            let _ = stdin.write_all(payload.as_bytes());
        }
    });
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    std::thread::spawn(move || read_capped(stdout.as_mut()));
    let (stderr_tx, stderr_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = stderr_tx.send(read_capped(stderr.as_mut()));
    });

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            kill_hook_tree(&mut child);
            let _ = child.wait();
            timed_out = true;
            break None;
        }
        std::thread::sleep(POLL_INTERVAL);
    };

    // A descendant of the hook can hold the pipe open after the hook itself is
    // gone, so the read is on a grace period rather than an open-ended join.
    let stderr = stderr_rx.recv_timeout(STDERR_GRACE).unwrap_or_default();

    Ok(HookRun {
        code: status.and_then(|status| status.code()),
        stderr,
        timed_out,
    })
}

fn read_capped<R: Read>(reader: Option<&mut R>) -> String {
    let Some(reader) = reader else {
        return String::new();
    };
    let mut buffer = Vec::new();
    let _ = reader
        .by_ref()
        .take(MAX_CAPTURE_BYTES as u64)
        .read_to_end(&mut buffer);
    let _ = std::io::copy(reader, &mut std::io::sink());
    String::from_utf8_lossy(&buffer).into_owned()
}

#[cfg(unix)]
fn shell_command(command: &str) -> Command {
    use std::os::unix::process::CommandExt as _;

    let mut shell = Command::new("/bin/sh");
    // Own process group so a timeout can take the hook's children with it.
    shell.arg("-c").arg(command).process_group(0);
    shell
}

#[cfg(not(unix))]
fn shell_command(command: &str) -> Command {
    let mut shell = Command::new("cmd");
    shell.arg("/C").arg(command);
    shell
}

#[cfg(unix)]
fn kill_hook_tree(child: &mut std::process::Child) {
    if let Ok(pid) = i32::try_from(child.id())
        && let Some(pid) = rustix::process::Pid::from_raw(pid)
    {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
    let _ = child.kill();
}

#[cfg(not(unix))]
fn kill_hook_tree(child: &mut std::process::Child) {
    let _ = child.kill();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runner(event_key: &str, matcher: &str, command: &str, timeout: Option<u64>) -> HookRunner {
        let entry = HookEntry {
            matcher: Some(matcher.to_string()),
            command: Some(command.to_string()),
            timeout_seconds: timeout,
        };
        let mut config = HooksConfig::default();
        match event_key {
            "PostToolUse" => config.post_tool_use = Some(vec![entry]),
            "UserPromptSubmit" => config.user_prompt_submit = Some(vec![entry]),
            "Stop" => config.stop = Some(vec![entry]),
            "SessionStart" => config.session_start = Some(vec![entry]),
            "SessionEnd" => config.session_end = Some(vec![entry]),
            "PreCompact" => config.pre_compact = Some(vec![entry]),
            "SubagentStop" => config.subagent_stop = Some(vec![entry]),
            "Notification" => config.notification = Some(vec![entry]),
            "Interrupt" => config.interrupt = Some(vec![entry]),
            _ => config.pre_tool_use = Some(vec![entry]),
        }
        HookRunner::new(config)
    }

    /// The panic path reaches every registered session, so two lifecycle tests
    /// running at once would see each other's `SessionEnd`.
    static LIFECYCLE: Mutex<()> = Mutex::new(());

    fn scratch(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("kesa-hooks-{name}-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    fn appender(path: &std::path::Path) -> String {
        format!("cat >> {}", path.display())
    }

    fn fired(path: &std::path::Path) -> String {
        std::fs::read_to_string(path).unwrap_or_default()
    }

    fn lifecycle_runner(path: &std::path::Path) -> Arc<HookRunner> {
        let entry = HookEntry {
            matcher: None,
            command: Some(appender(path)),
            timeout_seconds: Some(10),
        };
        Arc::new(HookRunner::new(HooksConfig {
            session_end: Some(vec![entry]),
            ..HooksConfig::default()
        }))
    }

    #[test]
    fn exit_two_blocks_and_returns_stderr_to_the_caller() {
        asupersync::test_utils::run_test(|| async {
            let runner = runner(
                "PreToolUse",
                "bash",
                "printf 'no shell commands today' >&2; exit 2",
                Some(10),
            );

            let decision = runner.pre_tool_use("bash", &json!({"command": "ls"})).await;

            assert_eq!(
                decision,
                HookDecision::Block {
                    reason: "no shell commands today".to_string()
                }
            );
        });
    }

    #[test]
    fn hook_receives_the_event_as_json_on_stdin() {
        asupersync::test_utils::run_test(|| async {
            let runner = runner(
                "PreToolUse",
                "*",
                "grep -q '\"tool_name\":\"bash\"' && { printf saw-payload >&2; exit 2; }",
                Some(10),
            );

            let decision = runner.pre_tool_use("bash", &json!({"command": "ls"})).await;

            assert_eq!(
                decision,
                HookDecision::Block {
                    reason: "saw-payload".to_string()
                }
            );
        });
    }

    #[test]
    fn exit_one_warns_and_allows() {
        asupersync::test_utils::run_test(|| async {
            let runner = runner("PreToolUse", "bash", "echo broken >&2; exit 1", Some(10));

            let decision = runner.pre_tool_use("bash", &json!({})).await;

            assert_eq!(decision, HookDecision::Allow);
        });
    }

    #[test]
    fn timeout_kills_the_child_and_allows() {
        asupersync::test_utils::run_test(|| async {
            let runner = runner("PreToolUse", "bash", "sleep 30; exit 2", Some(1));

            let started = Instant::now();
            let decision = runner.pre_tool_use("bash", &json!({})).await;

            assert_eq!(decision, HookDecision::Allow);
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "hook was not killed at its timeout: {:?}",
                started.elapsed()
            );
        });
    }

    #[test]
    fn matcher_miss_never_runs_the_hook() {
        asupersync::test_utils::run_test(|| async {
            let runner = runner("PreToolUse", "bash", "exit 2", Some(10));

            let decision = runner.pre_tool_use("read", &json!({})).await;

            assert_eq!(decision, HookDecision::Allow);
        });
    }

    #[test]
    fn no_hooks_configured_runs_nothing() {
        asupersync::test_utils::run_test(|| async {
            let runner = HookRunner::default();

            assert!(runner.is_empty());
            assert_eq!(
                runner.pre_tool_use("bash", &json!({})).await,
                HookDecision::Allow
            );
            assert_eq!(runner.stop(false).await, HookDecision::Allow);
        });
    }

    #[test]
    fn post_tool_use_and_stop_use_their_own_lists() {
        asupersync::test_utils::run_test(|| async {
            let post = runner(
                "PostToolUse",
                "*",
                "printf 'lint failed' >&2; exit 2",
                Some(10),
            );
            assert_eq!(
                post.post_tool_use("write", &json!({}), &json!({"content": "x"}))
                    .await,
                HookDecision::Block {
                    reason: "lint failed".to_string()
                }
            );
            // A PostToolUse list must not fire on PreToolUse.
            assert_eq!(
                post.pre_tool_use("write", &json!({})).await,
                HookDecision::Allow
            );

            let stop = runner("Stop", "*", "printf 'keep going' >&2; exit 2", Some(10));
            assert_eq!(
                stop.stop(false).await,
                HookDecision::Block {
                    reason: "keep going".to_string()
                }
            );

            let prompt = runner("UserPromptSubmit", "*", "exit 2", Some(10));
            assert!(matches!(
                prompt.user_prompt_submit("hello").await,
                HookDecision::Block { .. }
            ));
        });
    }

    #[test]
    fn matcher_hits_and_misses() {
        assert!(matcher_matches("*", "bash"));
        assert!(matcher_matches("", "bash"));
        assert!(matcher_matches("bash", "Bash"));
        assert!(matcher_matches("edit|write", "write"));
        assert!(matcher_matches("mcp__*", "mcp__github__list"));
        assert!(matcher_matches("*edit", "str_replace_edit"));
        assert!(matcher_matches("web*fetch", "web_fetch"));

        assert!(!matcher_matches("bash", "bashful"));
        assert!(!matcher_matches("edit|write", "read"));
        assert!(!matcher_matches("mcp__*", "bash"));
        assert!(!matcher_matches("*edit", "edit_file"));
    }

    #[test]
    fn entries_default_to_empty_and_specific_matcher_skips_unmatched_events() {
        let config = HooksConfig::default();
        assert!(!config.has_hooks());
        assert!(config.entries(HookEvent::PreToolUse).is_empty());

        let parsed: HooksConfig = serde_json::from_str(
            r#"{"PreToolUse":[{"matcher":"bash","command":"true","timeout":3}]}"#,
        )
        .expect("parse hooks config");
        assert!(parsed.has_hooks());
        assert_eq!(parsed.entries(HookEvent::PreToolUse).len(), 1);
        assert_eq!(
            parsed.entries(HookEvent::PreToolUse)[0].timeout_seconds,
            Some(3)
        );
        assert!(parsed.entries(HookEvent::Stop).is_empty());
    }

    #[test]
    fn each_new_event_reads_its_own_list() {
        asupersync::test_utils::run_test(|| async {
            let start = runner("SessionStart", "*", "printf started >&2; exit 2", Some(10));
            assert!(matches!(
                start.session_start("s1", "startup").await,
                HookDecision::Block { .. }
            ));
            assert_eq!(start.stop(false).await, HookDecision::Allow);

            let compact = runner("PreCompact", "*", "printf compacting >&2; exit 2", Some(10));
            assert_eq!(
                compact.pre_compact("auto", None).await,
                HookDecision::Block {
                    reason: "compacting".to_string()
                }
            );

            let subagent = runner("SubagentStop", "*", "printf child >&2; exit 2", Some(10));
            assert_eq!(
                subagent.subagent_stop(false).await,
                HookDecision::Block {
                    reason: "child".to_string()
                }
            );

            let notify = runner("Notification", "*", "printf noted >&2; exit 2", Some(10));
            assert_eq!(
                notify.notification("done").await,
                HookDecision::Block {
                    reason: "noted".to_string()
                }
            );
        });
    }

    #[test]
    fn every_event_parses_from_settings_under_all_three_spellings() {
        let parsed: HooksConfig = serde_json::from_str(
            r#"{
                "PreToolUse":[{"command":"true"}],
                "postToolUse":[{"command":"true"}],
                "user_prompt_submit":[{"command":"true"}],
                "Stop":[{"command":"true"}],
                "SessionStart":[{"command":"true"}],
                "sessionEnd":[{"command":"true"}],
                "pre_compact":[{"command":"true"}],
                "SubagentStop":[{"command":"true"}],
                "notification":[{"command":"true"}],
                "Interrupt":[{"command":"true"}]
            }"#,
        )
        .expect("parse hooks config");

        for event in HookEvent::ALL {
            assert_eq!(
                parsed.entries(event).len(),
                1,
                "{} did not read its own list",
                event.as_str()
            );
        }
    }

    #[test]
    fn merging_two_settings_files_keeps_every_event() {
        let base: HooksConfig = serde_json::from_str(
            r#"{"SessionEnd":[{"command":"a"}],"Interrupt":[{"command":"i1"}]}"#,
        )
        .expect("base");
        let other: HooksConfig = serde_json::from_str(
            r#"{"SessionEnd":[{"command":"b"}],"PreCompact":[{"command":"c"}],"interrupt":[{"command":"i2"}]}"#,
        )
        .expect("other");

        let merged = HooksConfig::merged(base, other);

        assert_eq!(merged.entries(HookEvent::SessionEnd).len(), 2);
        assert_eq!(merged.entries(HookEvent::PreCompact).len(), 1);
        assert_eq!(merged.entries(HookEvent::Interrupt).len(), 2);
    }

    #[test]
    fn session_end_fires_once_when_the_session_is_dropped() {
        let _serialized = LIFECYCLE.lock().unwrap_or_else(PoisonError::into_inner);
        let path = scratch("exit");

        let lifecycle = SessionLifecycle::watch(lifecycle_runner(&path), "sess-exit");
        assert_eq!(
            fired(&path),
            "",
            "SessionEnd fired before the session ended"
        );
        drop(lifecycle);

        let payload = fired(&path);
        assert!(payload.contains(r#""reason":"exit""#), "got {payload:?}");
        assert!(
            payload.contains(r#""session_id":"sess-exit""#),
            "got {payload:?}"
        );
        assert_eq!(payload.matches("SessionEnd").count(), 1, "got {payload:?}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn session_end_fires_on_a_panic_and_lets_the_panic_through() {
        let _serialized = LIFECYCLE.lock().unwrap_or_else(PoisonError::into_inner);
        let path = scratch("panic");

        let lifecycle = SessionLifecycle::watch(lifecycle_runner(&path), "sess-panic");
        let unwound = std::panic::catch_unwind(|| panic!("hook test panic, not a failure"));

        assert!(unwound.is_err(), "the panic was swallowed");
        let payload = fired(&path);
        assert!(payload.contains(r#""reason":"panic""#), "got {payload:?}");

        // Dropping now must not fire a second SessionEnd.
        drop(lifecycle);
        assert_eq!(fired(&path).matches("SessionEnd").count(), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn an_explicit_end_wins_and_the_drop_stays_quiet() {
        let _serialized = LIFECYCLE.lock().unwrap_or_else(PoisonError::into_inner);
        let path = scratch("interrupt");

        let lifecycle = SessionLifecycle::watch(lifecycle_runner(&path), "sess-int");
        lifecycle.end(SessionEndReason::Interrupt);
        drop(lifecycle);

        let payload = fired(&path);
        assert!(
            payload.contains(r#""reason":"interrupt""#),
            "got {payload:?}"
        );
        assert_eq!(payload.matches("SessionEnd").count(), 1, "got {payload:?}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_session_with_no_end_hook_leaves_the_process_alone() {
        let _serialized = LIFECYCLE.lock().unwrap_or_else(PoisonError::into_inner);
        let lifecycle = SessionLifecycle::watch(Arc::new(HookRunner::default()), "sess-none");

        assert!(
            ACTIVE
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .is_empty(),
            "an unconfigured session registered for the panic and signal paths"
        );
        drop(lifecycle);
    }
}
