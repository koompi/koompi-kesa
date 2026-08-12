//! Permission rules for built-in and extension tool calls.
//!
//! Distinct from [`crate::permissions`], which persists per-extension capability
//! grants. This module decides whether a single tool call may run at all.

use crate::config::PermissionsConfig;
use crate::tools::ToolEffects;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionMode {
    #[default]
    Default,
    AcceptEdits,
    Plan,
    ReadOnly,
}

impl PermissionMode {
    pub fn next(self) -> Self {
        match self {
            Self::Default => Self::AcceptEdits,
            Self::AcceptEdits => Self::Plan,
            Self::Plan => Self::ReadOnly,
            Self::ReadOnly => Self::Default,
        }
    }

    pub fn indicator(self) -> Option<&'static str> {
        match self {
            Self::Default => None,
            Self::AcceptEdits => Some("\u{23f5}\u{23f5} accept edits on"),
            Self::Plan => Some("\u{23f5}\u{23f5} plan mode on"),
            Self::ReadOnly => Some("\u{23f5}\u{23f5} read-only on"),
        }
    }
}

impl fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Default => "default",
            Self::AcceptEdits => "accept-edits",
            Self::Plan => "plan",
            Self::ReadOnly => "read-only",
        })
    }
}

/// Outcome of evaluating one tool call against the policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny { reason: String },
    Ask,
}

impl Decision {
    fn deny(reason: impl Into<String>) -> Self {
        Self::Deny {
            reason: reason.into(),
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RuleError {
    #[error("rule `{0}` is empty")]
    Empty(String),
    #[error("rule `{0}` is missing a closing `)`")]
    Unterminated(String),
    #[error("rule `{0}` has no tool name before `(`")]
    NoToolName(String),
}

/// Which input field a pattern is matched against, and how its glob segments
/// are delimited. Every tool's argument mapping is decided here and nowhere
/// else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetKind {
    /// Shell command line. Normalised before matching, no segment separator.
    Command,
    /// Filesystem path. `*` stops at `/`, `**` crosses it.
    Path,
}

impl TargetKind {
    fn separator(self) -> Option<char> {
        match self {
            Self::Command => None,
            Self::Path => Some('/'),
        }
    }
}

const COMMAND_KEYS: &[&str] = &["command"];
const PATH_KEYS: &[&str] = &["path", "file_path"];

fn string_field(input: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| input.get(*key).and_then(Value::as_str))
        .map(str::to_owned)
}

fn match_target(tool_name: &str, input: &Value) -> Option<(TargetKind, String)> {
    let command = |input: &Value| {
        string_field(input, COMMAND_KEYS).map(|cmd| {
            (
                TargetKind::Command,
                normalize_command(&cmd.to_ascii_lowercase()),
            )
        })
    };
    let path = |input: &Value| string_field(input, PATH_KEYS).map(|p| (TargetKind::Path, p));

    match tool_name {
        "bash" => command(input),
        "read" | "edit" | "write" | "grep" | "find" | "ls" | "hashline_edit" => path(input),
        "subagent" => None,
        // Extension tools register arbitrary names at runtime, so fall back to
        // the shape of their input rather than a name table.
        _ => command(input).or_else(|| path(input)),
    }
}

/// Fold the shell spellings that evaluate to whitespace or quoting into their
/// plain form, so `rm${IFS}-rf${IFS}/` cannot walk past a `Bash(rm:*)` rule.
///
/// Mirrors `normalize_command_for_classification` in
/// `extensions::exec_mediation`, which is private to that module.
fn normalize_command(command: &str) -> String {
    let mut normalized = String::with_capacity(command.len());
    let mut previous_was_space = false;
    let mut remaining = command;

    while !remaining.is_empty() {
        if let Some(rest) = remaining
            .strip_prefix("${ifs}")
            .or_else(|| remaining.strip_prefix("$ifs"))
        {
            if !previous_was_space {
                normalized.push(' ');
                previous_was_space = true;
            }
            remaining = rest;
            continue;
        }

        let mut chars = remaining.chars();
        let Some(mut ch) = chars.next() else {
            break;
        };

        if ch == '\'' || ch == '"' {
            remaining = chars.as_str();
            continue;
        }

        if ch == '\\' {
            let mut peek = chars.clone();
            if let Some(next) = peek.next() {
                if next == '\n' || next == '\r' {
                    remaining = peek.as_str();
                    continue;
                }
                chars.next();
                if next.is_ascii_whitespace() {
                    if !previous_was_space {
                        normalized.push(' ');
                        previous_was_space = true;
                    }
                    remaining = chars.as_str();
                    continue;
                }
                if next == '\'' || next == '"' {
                    remaining = chars.as_str();
                    continue;
                }
                ch = next;
            }
        }

        if ch.is_ascii_whitespace() {
            if !previous_was_space {
                normalized.push(' ');
                previous_was_space = true;
            }
        } else {
            normalized.push(ch);
            previous_was_space = false;
        }
        remaining = chars.as_str();
    }

    normalized
}

fn glob_match(pattern: &[char], text: &[char], separator: Option<char>) -> bool {
    match pattern.first() {
        None => text.is_empty(),
        Some('*') => {
            let crosses_separator = pattern.get(1) == Some(&'*');
            let rest = if crosses_separator {
                &pattern[2..]
            } else {
                &pattern[1..]
            };
            if glob_match(rest, text, separator) {
                return true;
            }
            for (index, ch) in text.iter().enumerate() {
                if !crosses_separator && Some(*ch) == separator {
                    break;
                }
                if glob_match(rest, &text[index + 1..], separator) {
                    return true;
                }
            }
            false
        }
        Some(first) => {
            !text.is_empty()
                && text[0] == *first
                && glob_match(&pattern[1..], &text[1..], separator)
        }
    }
}

/// The argument half of a rule. `prefix:glob` anchors `prefix` at a word
/// boundary, which is what keeps `Bash(git commit:*)` off `git commit-tree`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pattern {
    prefix: Option<String>,
    glob: Vec<char>,
}

impl Pattern {
    fn parse(raw: &str) -> Self {
        match raw.split_once(':') {
            Some((prefix, glob)) => Self {
                prefix: Some(prefix.trim().to_ascii_lowercase()),
                glob: glob.chars().collect(),
            },
            None => Self {
                prefix: None,
                glob: raw.chars().collect(),
            },
        }
    }

    fn matches(&self, kind: TargetKind, target: &str) -> bool {
        let Some(prefix) = &self.prefix else {
            return glob_match(
                &self.glob,
                &target.chars().collect::<Vec<_>>(),
                kind.separator(),
            );
        };
        let Some(rest) = target.strip_prefix(prefix.as_str()) else {
            return false;
        };
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            return false;
        }
        glob_match(
            &self.glob,
            &rest.trim_start().chars().collect::<Vec<_>>(),
            kind.separator(),
        )
    }
}

/// One `ToolName(pattern)` or bare `ToolName` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    tool: String,
    pattern: Option<Pattern>,
    source: String,
}

impl Rule {
    pub fn parse(raw: &str) -> Result<Self, RuleError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(RuleError::Empty(raw.to_string()));
        }
        let Some(open) = trimmed.find('(') else {
            return Ok(Self {
                tool: trimmed.to_ascii_lowercase(),
                pattern: None,
                source: trimmed.to_string(),
            });
        };
        if !trimmed.ends_with(')') {
            return Err(RuleError::Unterminated(raw.to_string()));
        }
        let tool = trimmed[..open].trim();
        if tool.is_empty() {
            return Err(RuleError::NoToolName(raw.to_string()));
        }
        Ok(Self {
            tool: tool.to_ascii_lowercase(),
            pattern: Some(Pattern::parse(&trimmed[open + 1..trimmed.len() - 1])),
            source: trimmed.to_string(),
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    fn matches(
        &self,
        tool_name: &str,
        target: Option<&(TargetKind, String)>,
        fail_closed: bool,
    ) -> bool {
        if self.tool != tool_name {
            return false;
        }
        let Some(pattern) = &self.pattern else {
            return true;
        };
        match target {
            Some((kind, value)) => pattern.matches(*kind, value),
            None => fail_closed,
        }
    }
}

fn parse_rules(raw: Option<&Vec<String>>) -> Result<Vec<Rule>, RuleError> {
    raw.map(|rules| rules.iter().map(|rule| Rule::parse(rule)).collect())
        .unwrap_or_else(|| Ok(Vec::new()))
}

/// Compiled rules plus the active mode. The single entry point J04 and J05 call.
#[derive(Debug, Clone, Default)]
pub struct ToolPolicy {
    mode: PermissionMode,
    deny: Vec<Rule>,
    allow: Vec<Rule>,
    ask: Vec<Rule>,
}

impl ToolPolicy {
    pub fn from_config(config: Option<&PermissionsConfig>) -> Result<Self, RuleError> {
        let Some(config) = config else {
            return Ok(Self::default());
        };
        Ok(Self {
            mode: config.mode.unwrap_or_default(),
            deny: parse_rules(config.deny.as_ref())?,
            allow: parse_rules(config.allow.as_ref())?,
            ask: parse_rules(config.ask.as_ref())?,
        })
    }

    pub fn mode(&self) -> PermissionMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: PermissionMode) {
        self.mode = mode;
    }

    /// Add an allow rule for the rest of this process. Backs the approval
    /// modal's "don't ask again" option; nothing here touches settings.json,
    /// so a session grant dies with the session.
    pub fn allow_for_session(&mut self, rule: &str) -> Result<(), RuleError> {
        self.allow.push(Rule::parse(rule)?);
        Ok(())
    }

    /// Deny beats allow beats ask beats the mode default. A user who wrote a
    /// deny rule and got prompted anyway would read the prompt as the bug.
    pub fn decide(&self, tool_name: &str, effects: ToolEffects, input: &Value) -> Decision {
        let tool_name = tool_name.to_ascii_lowercase();
        let target = match_target(&tool_name, input);

        if let Some(rule) = self
            .deny
            .iter()
            .find(|rule| rule.matches(&tool_name, target.as_ref(), true))
        {
            return Decision::deny(format!("denied by permission rule `{}`", rule.source));
        }
        if self
            .allow
            .iter()
            .any(|rule| rule.matches(&tool_name, target.as_ref(), false))
        {
            return Decision::Allow;
        }
        if self
            .ask
            .iter()
            .any(|rule| rule.matches(&tool_name, target.as_ref(), false))
        {
            return Decision::Ask;
        }
        mode_default(self.mode, &tool_name, effects)
    }
}

/// Mode defaults keyed on declared effects, never on tool name: extension tools
/// register names this crate has never seen, and the `Tool::effects` default is
/// `write()`, so an undeclared one lands in the mutating row fail-closed.
fn mode_default(mode: PermissionMode, tool_name: &str, effects: ToolEffects) -> Decision {
    let denied = |what: &str| {
        Decision::deny(format!(
            "{mode} mode does not allow `{tool_name}`, which {what}"
        ))
    };

    if effects.processes() {
        return match mode {
            PermissionMode::Plan | PermissionMode::ReadOnly => denied("starts a process"),
            PermissionMode::Default | PermissionMode::AcceptEdits => Decision::Ask,
        };
    }
    if effects.writes() || effects.appends() {
        return match mode {
            PermissionMode::Plan | PermissionMode::ReadOnly => denied("modifies files"),
            PermissionMode::Default => Decision::Ask,
            PermissionMode::AcceptEdits => Decision::Allow,
        };
    }
    if effects.networks() {
        return match mode {
            PermissionMode::ReadOnly => denied("performs network I/O"),
            _ => Decision::Ask,
        };
    }
    Decision::Allow
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn read() -> ToolEffects {
        ToolEffects::read()
    }

    fn bash() -> ToolEffects {
        ToolEffects::process().union(ToolEffects::write())
    }

    fn edit() -> ToolEffects {
        ToolEffects::write()
    }

    fn policy(deny: &[&str], allow: &[&str], ask: &[&str], mode: PermissionMode) -> ToolPolicy {
        let to_vec = |rules: &[&str]| Some(rules.iter().map(|r| (*r).to_string()).collect());
        ToolPolicy::from_config(Some(&PermissionsConfig {
            mode: Some(mode),
            deny: to_vec(deny),
            allow: to_vec(allow),
            ask: to_vec(ask),
        }))
        .expect("rules parse")
    }

    #[test]
    fn mode_cycle_returns_to_start_after_four_steps() {
        let mut mode = PermissionMode::Default;
        for _ in 0..4 {
            mode = mode.next();
        }
        assert_eq!(mode, PermissionMode::Default);
    }

    #[test]
    fn default_mode_has_no_indicator() {
        assert_eq!(PermissionMode::Default.indicator(), None);
        assert_eq!(
            PermissionMode::AcceptEdits.indicator(),
            Some("\u{23f5}\u{23f5} accept edits on")
        );
    }

    #[test]
    fn bare_rule_matches_every_call_to_that_tool() {
        let rule = Rule::parse("Edit").expect("parses");
        assert!(rule.matches("edit", Some(&(TargetKind::Path, "a.rs".into())), false));
        assert!(!rule.matches("write", Some(&(TargetKind::Path, "a.rs".into())), false));
    }

    #[test]
    fn malformed_rules_are_rejected() {
        assert_eq!(
            Rule::parse("Bash(rm:*"),
            Err(RuleError::Unterminated("Bash(rm:*".into()))
        );
        assert_eq!(Rule::parse("  "), Err(RuleError::Empty("  ".into())));
        assert_eq!(
            Rule::parse("(rm:*)"),
            Err(RuleError::NoToolName("(rm:*)".into()))
        );
    }

    #[test]
    fn single_star_stops_at_a_path_separator_and_double_star_crosses_it() {
        let shallow = Rule::parse("Read(src/*)").expect("parses");
        let deep = Rule::parse("Read(src/**)").expect("parses");
        let nested = (TargetKind::Path, "src/a/b.rs".to_string());
        let flat = (TargetKind::Path, "src/a.rs".to_string());
        assert!(!shallow.matches("read", Some(&nested), false));
        assert!(shallow.matches("read", Some(&flat), false));
        assert!(deep.matches("read", Some(&nested), false));
    }

    #[test]
    fn subagent_has_no_match_target_so_pattern_rules_do_not_allow_it() {
        let policy = policy(&[], &["Subagent(anything:*)"], &[], PermissionMode::Default);
        assert_eq!(
            policy.decide("subagent", ToolEffects::process(), &json!({"prompt": "hi"})),
            Decision::Ask
        );
    }

    #[test]
    fn a_deny_rule_with_no_extractable_target_fails_closed() {
        let policy = policy(&["Bash(rm:*)"], &[], &[], PermissionMode::Default);
        assert!(matches!(
            policy.decide("bash", bash(), &json!({})),
            Decision::Deny { .. }
        ));
    }

    #[test]
    fn extension_tools_are_decided_by_effects_not_by_name() {
        let policy = policy(&[], &[], &[], PermissionMode::ReadOnly);
        let unknown_writer = policy.decide("acme_deploy", ToolEffects::write(), &json!({}));
        let unknown_reader = policy.decide("acme_lookup", ToolEffects::read(), &json!({}));
        assert!(matches!(unknown_writer, Decision::Deny { .. }));
        assert_eq!(unknown_reader, Decision::Allow);
    }

    #[test]
    fn ask_rules_override_an_auto_allowing_mode() {
        let policy = policy(&[], &[], &["Edit"], PermissionMode::AcceptEdits);
        assert_eq!(
            policy.decide("edit", edit(), &json!({"path": "a.rs"})),
            Decision::Ask
        );
    }

    #[test]
    fn global_and_project_settings_both_contribute_rules() {
        use crate::config::Config;

        let global_dir = tempfile::tempdir().expect("global tempdir");
        let project_dir = tempfile::tempdir().expect("project tempdir");
        std::fs::write(
            global_dir.path().join("settings.json"),
            r#"{"permissions": {"mode": "plan", "deny": ["Bash(rm:*)"]}}"#,
        )
        .expect("write global settings");
        let project_settings = project_dir.path().join(".kode");
        std::fs::create_dir_all(&project_settings).expect("create project dir");
        std::fs::write(
            project_settings.join("settings.json"),
            r#"{"permissions": {"allow": ["Bash(git commit:*)"]}}"#,
        )
        .expect("write project settings");

        let config = Config::load_with_roots(None, global_dir.path(), project_dir.path())
            .expect("load settings");
        let permissions = config.permissions.expect("permissions section");
        assert_eq!(
            permissions.deny.as_deref(),
            Some(&["Bash(rm:*)".to_string()][..])
        );
        assert_eq!(
            permissions.allow.as_deref(),
            Some(&["Bash(git commit:*)".to_string()][..])
        );
        assert_eq!(permissions.mode, Some(PermissionMode::Plan));

        let policy = ToolPolicy::from_config(Some(&permissions)).expect("rules parse");
        assert!(matches!(
            policy.decide("bash", bash(), &json!({"command": "rm -rf /tmp/x"})),
            Decision::Deny { .. }
        ));
        assert_eq!(
            policy.decide("bash", bash(), &json!({"command": "git commit -m x"})),
            Decision::Allow
        );
    }

    struct Case {
        rules: &'static str,
        mode: PermissionMode,
        tool: &'static str,
        effects: ToolEffects,
        input: Value,
        expected: Decision,
    }

    fn decision_label(decision: &Decision) -> &'static str {
        match decision {
            Decision::Allow => "Allow",
            Decision::Deny { .. } => "Deny",
            Decision::Ask => "Ask",
        }
    }

    #[test]
    fn policy_decision_table() {
        let deny_rm = || policy(&["Bash(rm:*)"], &[], &[], PermissionMode::Default);
        let allow_commit = || policy(&[], &["Bash(git commit:*)"], &[], PermissionMode::Default);
        let deny_beats_allow = || {
            policy(
                &["Bash(git push:*)"],
                &["Bash(git:*)"],
                &[],
                PermissionMode::Default,
            )
        };

        let cases = vec![
            Case {
                rules: "allow Bash(git commit:*)",
                mode: PermissionMode::Default,
                tool: "bash",
                effects: bash(),
                input: json!({"command": "git commit -m x"}),
                expected: Decision::Allow,
            },
            Case {
                rules: "allow Bash(git commit:*)",
                mode: PermissionMode::Default,
                tool: "bash",
                effects: bash(),
                input: json!({"command": "git commit-tree HEAD"}),
                expected: Decision::Ask,
            },
            Case {
                rules: "deny Bash(rm:*)",
                mode: PermissionMode::Default,
                tool: "bash",
                effects: bash(),
                input: json!({"command": "rm${IFS}-rf${IFS}/"}),
                expected: Decision::deny(""),
            },
            Case {
                rules: "deny Bash(rm:*)",
                mode: PermissionMode::Default,
                tool: "bash",
                effects: bash(),
                input: json!({"command": "rmdir stale"}),
                expected: Decision::Ask,
            },
            Case {
                rules: "deny Bash(git push:*) + allow Bash(git:*)",
                mode: PermissionMode::Default,
                tool: "bash",
                effects: bash(),
                input: json!({"command": "git push --force"}),
                expected: Decision::deny(""),
            },
            Case {
                rules: "deny Bash(git push:*) + allow Bash(git:*)",
                mode: PermissionMode::Default,
                tool: "bash",
                effects: bash(),
                input: json!({"command": "git status"}),
                expected: Decision::Allow,
            },
        ];

        let mode_cases = [
            (PermissionMode::Default, Decision::Ask, Decision::Ask),
            (PermissionMode::AcceptEdits, Decision::Ask, Decision::Allow),
            (PermissionMode::Plan, Decision::deny(""), Decision::deny("")),
            (
                PermissionMode::ReadOnly,
                Decision::deny(""),
                Decision::deny(""),
            ),
        ];

        let cases = cases
            .into_iter()
            .chain(mode_cases.into_iter().flat_map(|(mode, on_bash, on_edit)| {
                [
                    Case {
                        rules: "none",
                        mode,
                        tool: "bash",
                        effects: bash(),
                        input: json!({"command": "ls"}),
                        expected: on_bash,
                    },
                    Case {
                        rules: "none",
                        mode,
                        tool: "edit",
                        effects: edit(),
                        input: json!({"path": "src/a.rs"}),
                        expected: on_edit,
                    },
                    Case {
                        rules: "none",
                        mode,
                        tool: "read",
                        effects: read(),
                        input: json!({"path": "src/a.rs"}),
                        expected: Decision::Allow,
                    },
                ]
            }))
            .collect::<Vec<_>>();

        println!(
            "\n{:<38} | {:<12} | {:<6} | {:<28} | {:<8} | {:<8}",
            "rule set", "mode", "tool", "input", "expected", "actual"
        );
        let mut failures = Vec::new();
        for case in &cases {
            let policy = match case.rules {
                "deny Bash(rm:*)" => deny_rm(),
                "allow Bash(git commit:*)" => allow_commit(),
                "deny Bash(git push:*) + allow Bash(git:*)" => deny_beats_allow(),
                _ => policy(&[], &[], &[], case.mode),
            };
            let mut policy = policy;
            policy.set_mode(case.mode);
            let actual = policy.decide(case.tool, case.effects, &case.input);
            let input = serde_json::to_string(&case.input).expect("input serialises");
            println!(
                "{:<38} | {:<12} | {:<6} | {:<28} | {:<8} | {:<8}",
                case.rules,
                case.mode.to_string(),
                case.tool,
                input,
                decision_label(&case.expected),
                decision_label(&actual)
            );
            if decision_label(&actual) != decision_label(&case.expected) {
                failures.push(format!("{} / {} / {input}", case.rules, case.tool));
            }
        }
        assert!(failures.is_empty(), "table rows failed: {failures:?}");
    }
}
