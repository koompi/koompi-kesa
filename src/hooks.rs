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

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Seconds a hook may run before it is killed, when the entry sets no timeout.
pub const DEFAULT_TIMEOUT_SECS: u64 = 60;

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
}

impl HookEvent {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::Stop => "Stop",
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
///       { "matcher": "bash", "command": "~/.kode/deny-rm.sh", "timeout": 5 }
///     ]
///   }
/// }
/// ```
///
/// Every command here runs with the user's own privileges and is not sandboxed,
/// so only a settings file the user controls may supply one. Hooks from a
/// project's `.kode/settings.json` are ignored unless the *global* settings file
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
        };
        entries.as_deref().unwrap_or(&[])
    }

    #[must_use]
    pub fn has_hooks(&self) -> bool {
        [
            HookEvent::PreToolUse,
            HookEvent::PostToolUse,
            HookEvent::UserPromptSubmit,
            HookEvent::Stop,
        ]
        .iter()
        .any(|event| !self.entries(*event).is_empty())
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
    pub async fn stop(&self) -> HookDecision {
        self.dispatch(
            HookEvent::Stop,
            "",
            || json!({ "hook_event_name": HookEvent::Stop.as_str() }),
        )
        .await
    }

    async fn dispatch<F>(&self, event: HookEvent, matcher_key: &str, payload: F) -> HookDecision
    where
        F: FnOnce() -> Value,
    {
        let matched: Vec<HookEntry> = self
            .config
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
            .collect();

        if matched.is_empty() {
            return HookDecision::Allow;
        }

        let payload = payload();
        let payload = serde_json::to_string(&add_cwd(payload)).unwrap_or_else(|_| "{}".to_string());

        for entry in matched {
            let command = entry.command.unwrap_or_default();
            let timeout =
                Duration::from_secs(entry.timeout_seconds.unwrap_or(DEFAULT_TIMEOUT_SECS).max(1));
            let decision = run_hook(event, command, payload.clone(), timeout).await;
            if let HookDecision::Block { .. } = decision {
                return decision;
            }
        }

        HookDecision::Allow
    }
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
        Ok(run) if run.timed_out => {
            tracing::warn!(
                "hook `{logged}` for {} exceeded {}s and was killed",
                event.as_str(),
                timeout.as_secs()
            );
            HookDecision::Allow
        }
        Ok(run) => match run.code {
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
                    "hook `{logged}` for {} exited with {code:?}: {}",
                    event.as_str(),
                    run.stderr.trim()
                );
                HookDecision::Allow
            }
        },
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
            _ => config.pre_tool_use = Some(vec![entry]),
        }
        HookRunner::new(config)
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
            assert_eq!(runner.stop().await, HookDecision::Allow);
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
                stop.stop().await,
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
}
