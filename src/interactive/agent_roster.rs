//! Reader for the `pi.subagent.*` wire schemas, and the roster the TUI draws.
//!
//! The producer's types stay private to [`crate::subagents`]: these schemas are
//! versioned contracts, so the consumer takes the fields it renders and
//! tolerates the rest changing.

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub const PROGRESS_SCHEMA: &str = "pi.subagent.progress.v1";
pub const RESULT_SCHEMA: &str = "pi.subagent.result.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Starting,
    Running,
    Completed,
    Failed,
    Cancelled,
    #[serde(other)]
    Unknown,
}

impl AgentStatus {
    pub const fn is_running(self) -> bool {
        matches!(self, Self::Starting | Self::Running)
    }

    pub const fn is_failure(self) -> bool {
        matches!(self, Self::Failed)
    }

    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Starting => "\u{25cb}",
            Self::Running => "\u{25b8}",
            Self::Completed => "\u{2713}",
            Self::Failed => "\u{2717}",
            Self::Cancelled => "\u{2298}",
            Self::Unknown => "\u{00b7}",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRow {
    pub agent: String,
    #[serde(default)]
    pub step: Option<usize>,
    pub status: AgentStatus,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub resolved_model: Option<String>,
    #[serde(default)]
    pub elapsed_ms: Option<u64>,
    #[serde(default)]
    pub tokens: Option<u64>,
    #[serde(default)]
    pub output: String,
    #[serde(default)]
    pub error: Option<String>,
}

pub type RowKey = (Option<usize>, String);

impl AgentRow {
    pub fn key(&self) -> RowKey {
        (self.step, self.agent.clone())
    }

    /// What the child actually reached, falling back to what its definition
    /// asked for. A definition naming no model inherits the parent's default,
    /// which nothing on this side can name until the child reports back.
    pub fn model_label(&self) -> String {
        match (&self.provider, &self.resolved_model) {
            (Some(provider), Some(model)) => format!("{provider}/{model}"),
            (None, Some(model)) => model.clone(),
            _ => self
                .model
                .clone()
                .unwrap_or_else(|| "inherited".to_string()),
        }
    }

    pub fn elapsed_label(&self) -> String {
        let Some(ms) = self.elapsed_ms else {
            return String::new();
        };
        let secs = ms / 1000;
        if secs < 60 {
            format!("{secs}s")
        } else {
            format!("{}m{:02}s", secs / 60, secs % 60)
        }
    }

    /// The newest thing this child said, or why it stopped saying anything.
    pub fn tail(&self) -> String {
        if let Some(error) = &self.error {
            return error.trim().replace('\n', " ");
        }
        self.output
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default()
            .trim()
            .to_string()
    }
}

fn schema_is(details: &Value, schema: &str) -> bool {
    details.get("schema").and_then(Value::as_str) == Some(schema)
}

pub fn progress_row(details: &Value) -> Option<AgentRow> {
    if !schema_is(details, PROGRESS_SCHEMA) {
        return None;
    }
    serde_json::from_value(details.get("result")?.clone()).ok()
}

#[derive(Debug, Clone)]
pub struct Delegation {
    pub mode: String,
    pub rows: Vec<AgentRow>,
}

impl Delegation {
    pub fn elapsed_label(&self) -> String {
        let total = match self.mode.as_str() {
            "chain" => self.rows.iter().filter_map(|row| row.elapsed_ms).sum(),
            _ => self
                .rows
                .iter()
                .filter_map(|row| row.elapsed_ms)
                .max()
                .unwrap_or(0),
        };
        AgentRow {
            elapsed_ms: Some(total),
            ..AgentRow::empty()
        }
        .elapsed_label()
    }
}

impl AgentRow {
    fn empty() -> Self {
        Self {
            agent: String::new(),
            step: None,
            status: AgentStatus::Unknown,
            model: None,
            provider: None,
            resolved_model: None,
            elapsed_ms: None,
            tokens: None,
            output: String::new(),
            error: None,
        }
    }
}

pub fn delegation(details: &Value) -> Option<Delegation> {
    if !schema_is(details, RESULT_SCHEMA) {
        return None;
    }
    let rows: Vec<AgentRow> = serde_json::from_value(details.get("results")?.clone()).ok()?;
    Some(Delegation {
        mode: details
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("single")
            .to_string(),
        rows,
    })
}

/// Distinguishes one child's stream from another's inside a single tool call.
/// Without it the batcher's newest-wins coalescing drops every agent but the
/// last one to speak, because all children share one update callback.
pub fn coalesce_suffix(details: Option<&Value>) -> Option<String> {
    let details = details?;
    if !schema_is(details, PROGRESS_SCHEMA) {
        return None;
    }
    let result = details.get("result")?;
    let agent = result.get("agent").and_then(Value::as_str)?;
    let step = result.get("step").and_then(Value::as_u64);
    Some(step.map_or_else(|| agent.to_string(), |step| format!("{step}\u{1f}{agent}")))
}

#[derive(Debug, Default)]
pub struct AgentRoster {
    rows: BTreeMap<RowKey, AgentRow>,
}

impl AgentRoster {
    pub fn apply(&mut self, row: AgentRow) {
        self.rows.insert(row.key(), row);
    }

    pub fn apply_all(&mut self, rows: Vec<AgentRow>) {
        for row in rows {
            self.apply(row);
        }
    }

    pub fn clear(&mut self) {
        self.rows.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn rows(&self) -> impl Iterator<Item = &AgentRow> {
        self.rows.values()
    }

    /// Running, finished, failed. Cancelled counts as neither finished nor
    /// failed: it is the one outcome the user chose.
    pub fn tally(&self) -> (usize, usize, usize) {
        let mut running = 0;
        let mut done = 0;
        let mut failed = 0;
        for row in self.rows.values() {
            match row.status {
                s if s.is_running() => running += 1,
                AgentStatus::Completed => done += 1,
                AgentStatus::Failed => failed += 1,
                _ => {}
            }
        }
        (running, done, failed)
    }

    pub fn summary_line(&self) -> String {
        let (running, done, failed) = self.tally();
        let mut parts = Vec::new();
        if running > 0 {
            parts.push(format!("{running} running"));
        }
        if done > 0 {
            parts.push(format!("{done} done"));
        }
        if failed > 0 {
            parts.push(format!("{failed} failed"));
        }
        if parts.is_empty() {
            "agents".to_string()
        } else {
            format!("agents \u{b7} {}", parts.join(" \u{b7} "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn progress(agent: &str, status: &str) -> Value {
        json!({
            "schema": PROGRESS_SCHEMA,
            "result": {
                "agent": agent,
                "task": "do a thing",
                "status": status,
                "provider": "anthropic",
                "resolvedModel": "claude-opus-5",
                "elapsedMs": 18_000,
                "output": "reading src/tools.rs\n",
                "cwd": "/tmp",
                "binary": "/usr/bin/kesa",
                "tools": [],
                "sessionIsolation": "ephemeral_no_session"
            }
        })
    }

    #[test]
    fn progress_row_reads_the_schema() {
        let row = progress_row(&progress("explorer", "running")).expect("row");
        assert_eq!(row.agent, "explorer");
        assert_eq!(row.status, AgentStatus::Running);
        assert_eq!(row.model_label(), "anthropic/claude-opus-5");
        assert_eq!(row.elapsed_label(), "18s");
        assert_eq!(row.tail(), "reading src/tools.rs");
    }

    #[test]
    fn a_foreign_schema_is_not_a_row() {
        assert!(progress_row(&json!({"schema": "kode.todo.list.v1"})).is_none());
    }

    #[test]
    fn an_unknown_status_does_not_lose_the_row() {
        let row = progress_row(&progress("explorer", "hibernating")).expect("row");
        assert_eq!(row.status, AgentStatus::Unknown);
    }

    #[test]
    fn model_label_falls_back_to_what_was_asked_for() {
        let mut row = progress_row(&progress("explorer", "running")).expect("row");
        row.provider = None;
        row.resolved_model = None;
        row.model = Some("gpt-5.5".to_string());
        assert_eq!(row.model_label(), "gpt-5.5");
        row.model = None;
        assert_eq!(row.model_label(), "inherited");
    }

    #[test]
    fn two_agents_keep_two_rows() {
        let mut roster = AgentRoster::default();
        roster.apply(progress_row(&progress("explorer", "running")).expect("row"));
        roster.apply(progress_row(&progress("reviewer", "completed")).expect("row"));
        roster.apply(progress_row(&progress("explorer", "completed")).expect("row"));
        assert_eq!(roster.rows().count(), 2);
        assert_eq!(roster.tally(), (0, 2, 0));
        assert_eq!(roster.summary_line(), "agents \u{b7} 2 done");
    }

    #[test]
    fn coalesce_suffix_separates_children_of_one_call() {
        let a = coalesce_suffix(Some(&progress("explorer", "running")));
        let b = coalesce_suffix(Some(&progress("reviewer", "running")));
        assert_ne!(a, b);
        assert_eq!(a, Some("explorer".to_string()));
        assert!(coalesce_suffix(Some(&json!({"schema": "other"}))).is_none());
    }

    #[test]
    fn delegation_reads_the_final_payload() {
        let details = json!({
            "schema": RESULT_SCHEMA,
            "mode": "parallel",
            "results": [
                {"agent": "explorer", "task": "t", "status": "completed", "elapsedMs": 42_000,
                 "output": "found 7 call sites", "cwd": "/tmp", "binary": "/b", "tools": [],
                 "sessionIsolation": "ephemeral_no_session"},
                {"agent": "tester", "task": "t", "status": "failed", "elapsedMs": 12_000,
                 "error": "child exited with code 1", "output": "", "cwd": "/tmp", "binary": "/b",
                 "tools": [], "sessionIsolation": "ephemeral_no_session"}
            ]
        });
        let delegation = delegation(&details).expect("delegation");
        assert_eq!(delegation.mode, "parallel");
        assert_eq!(delegation.rows.len(), 2);
        assert_eq!(delegation.elapsed_label(), "42s");
        assert_eq!(delegation.rows[1].tail(), "child exited with code 1");
    }

    #[test]
    fn a_chain_totals_its_steps_and_a_fan_out_takes_the_longest() {
        let rows = |mode: &str| Delegation {
            mode: mode.to_string(),
            rows: vec![
                AgentRow {
                    elapsed_ms: Some(30_000),
                    ..AgentRow::empty()
                },
                AgentRow {
                    elapsed_ms: Some(45_000),
                    ..AgentRow::empty()
                },
            ],
        };
        assert_eq!(rows("chain").elapsed_label(), "1m15s");
        assert_eq!(rows("parallel").elapsed_label(), "45s");
    }
}
