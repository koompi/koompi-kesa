//! The agent's own checklist, held in memory for the life of the process.
//!
//! Every write carries the whole list, so the tool never merges: it validates the
//! incoming list, replaces the stored one, and echoes it back as a checkbox block.

use crate::error::{Error, Result};
use crate::model::{ContentBlock, TextContent};
use crate::tools::{Tool, ToolEffects, ToolOutput, ToolUpdate};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::{Mutex, OnceLock};

/// Marks a `details` payload as a todo list so the TUI can render checkboxes.
pub const TODO_LIST_SCHEMA: &str = "kesa.todo.list.v1";

/// Pre-rename identifier. A session recorded before the rename still carries
/// it, and so does any extension not yet rebuilt.
pub const LEGACY_TODO_LIST_SCHEMA: &str = "kode.todo.list.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

impl TodoStatus {
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::Pending => '☐',
            Self::InProgress => '▣',
            Self::Completed => '☑',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
    #[serde(alias = "activeForm")]
    pub active_form: String,
}

impl TodoItem {
    /// The in-progress item reads as what is happening now; the rest as what to do.
    #[must_use]
    pub fn display_line(&self) -> String {
        let label = if self.status == TodoStatus::InProgress && !self.active_form.trim().is_empty()
        {
            self.active_form.trim()
        } else {
            self.content.trim()
        };
        format!("{} {label}", self.status.glyph())
    }
}

/// Render a list as the checkbox block shown in the transcript.
#[must_use]
pub fn render_todo_list(todos: &[TodoItem]) -> String {
    todos
        .iter()
        .map(TodoItem::display_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// One row for the panel above the editor: what is happening now, and how far
/// through the list it is. `None` when there is no plan to show.
#[must_use]
pub fn summary_line(todos: &[TodoItem]) -> Option<String> {
    if todos.is_empty() {
        return None;
    }
    let done = todos
        .iter()
        .filter(|item| item.status == TodoStatus::Completed)
        .count();
    let current = todos
        .iter()
        .find(|item| item.status == TodoStatus::InProgress)
        .or_else(|| todos.iter().find(|item| item.status == TodoStatus::Pending));
    let head = current.map_or_else(
        || format!("{} All done", TodoStatus::Completed.glyph()),
        TodoItem::display_line,
    );
    Some(format!("{head} ({done}/{})", todos.len()))
}

fn store() -> &'static Mutex<Vec<TodoItem>> {
    static STORE: OnceLock<Mutex<Vec<TodoItem>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Vec::new()))
}

/// The list as of the last successful write.
#[must_use]
pub fn current_todos() -> Vec<TodoItem> {
    store().lock().map_or_else(|_| Vec::new(), |t| t.clone())
}

fn validate(todos: &[TodoItem]) -> Result<()> {
    if let Some(index) = todos.iter().position(|item| item.content.trim().is_empty()) {
        return Err(Error::tool(
            "todo",
            format!(
                "Item {} has empty content; every item needs an imperative description.",
                index + 1
            ),
        ));
    }
    let in_progress = todos
        .iter()
        .filter(|item| item.status == TodoStatus::InProgress)
        .count();
    if in_progress > 1 {
        return Err(Error::tool(
            "todo",
            format!(
                "{in_progress} items are in_progress; exactly one item may be in progress at a time."
            ),
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct TodoWrite {
    todos: Vec<TodoItem>,
}

/// Tracks the plan for a multi-step task.
pub struct TodoTool;

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &'static str {
        "todo"
    }

    fn label(&self) -> &'static str {
        "Todos"
    }

    fn description(&self) -> &'static str {
        "Record and update the plan for the current task. Use it for work of three or more steps, and skip it for a single trivial change. Send the entire list on every call: it replaces the previous one, so omitting an item deletes it. Mark exactly one item in_progress while you work on it, and mark it completed as soon as it is done rather than batching completions."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "required": ["todos"],
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "The complete list, in order. An empty array clears the plan.",
                    "items": {
                        "type": "object",
                        "required": ["content", "status", "active_form"],
                        "properties": {
                            "content": {"type": "string", "description": "Imperative form, e.g. \"Run the tests\"."},
                            "status": {"type": "string", "enum": ["pending", "in_progress", "completed"]},
                            "active_form": {"type": "string", "description": "Present continuous form, e.g. \"Running the tests\"."}
                        },
                        "additionalProperties": false
                    }
                }
            },
            "additionalProperties": false
        })
    }

    async fn execute(
        &self,
        _tool_call_id: &str,
        input: Value,
        _on_update: Option<Box<dyn Fn(ToolUpdate) + Send + Sync>>,
    ) -> Result<ToolOutput> {
        let write: TodoWrite = serde_json::from_value(input)
            .map_err(|error| Error::tool("todo", format!("Invalid input: {error}")))?;
        validate(&write.todos)?;

        let text = if write.todos.is_empty() {
            "Todo list cleared.".to_string()
        } else {
            render_todo_list(&write.todos)
        };
        let details = json!({
            "schema": TODO_LIST_SCHEMA,
            "todos": write.todos,
        });
        if let Ok(mut stored) = store().lock() {
            *stored = write.todos;
        }

        Ok(ToolOutput {
            content: vec![ContentBlock::Text(TextContent::new(text))],
            details: Some(details),
            is_error: false,
        })
    }

    fn effects(&self) -> ToolEffects {
        ToolEffects::read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asupersync::runtime::RuntimeBuilder;

    fn item(content: &str, status: TodoStatus) -> TodoItem {
        TodoItem {
            content: content.to_string(),
            status,
            active_form: format!("{content}ing"),
        }
    }

    fn write(todos: Value) -> Result<ToolOutput> {
        RuntimeBuilder::current_thread()
            .build()
            .expect("runtime build")
            .block_on(async {
                TodoTool
                    .execute("call-1", json!({"todos": todos}), None)
                    .await
            })
    }

    #[test]
    fn rejects_two_in_progress_items() {
        let error = write(json!([
            {"content": "Read the file", "status": "in_progress", "active_form": "Reading the file"},
            {"content": "Run the tests", "status": "in_progress", "active_form": "Running the tests"},
        ]))
        .expect_err("two in_progress items must be rejected");
        assert!(error.to_string().contains("in progress"), "{error}");
    }

    #[test]
    fn rejects_empty_content() {
        let error = write(json!([
            {"content": "   ", "status": "pending", "active_form": "Doing it"},
        ]))
        .expect_err("empty content must be rejected");
        assert!(error.to_string().contains("empty content"), "{error}");
    }

    #[test]
    fn round_trips_three_items() {
        let output = write(json!([
            {"content": "Read the file", "status": "completed", "active_form": "Reading the file"},
            {"content": "Run the tests", "status": "in_progress", "active_form": "Running the tests"},
            {"content": "Write the report", "status": "pending", "active_form": "Writing the report"},
        ]))
        .expect("valid list");

        let details = output.details.expect("details");
        assert_eq!(details["schema"], TODO_LIST_SCHEMA);
        let stored: Vec<TodoItem> =
            serde_json::from_value(details["todos"].clone()).expect("todos round trip");
        assert_eq!(stored.len(), 3);
        assert_eq!(stored, current_todos());

        let rendered = render_todo_list(&stored);
        assert_eq!(
            rendered,
            "☑ Read the file\n▣ Running the tests\n☐ Write the report"
        );
    }

    #[test]
    fn accepts_camel_case_active_form() {
        let parsed: TodoItem = serde_json::from_value(
            json!({"content": "Run the tests", "status": "pending", "activeForm": "Running the tests"}),
        )
        .expect("activeForm alias");
        assert_eq!(parsed.active_form, "Running the tests");
    }

    #[test]
    fn summary_leads_with_the_item_in_progress() {
        let todos = vec![
            item("Read the file", TodoStatus::Completed),
            item("Run the tests", TodoStatus::InProgress),
            item("Write the report", TodoStatus::Pending),
        ];
        assert_eq!(
            summary_line(&todos).as_deref(),
            Some("▣ Run the testsing (1/3)")
        );
    }

    #[test]
    fn summary_falls_back_to_the_next_pending_item() {
        let todos = vec![
            item("Read the file", TodoStatus::Completed),
            item("Run the tests", TodoStatus::Pending),
        ];
        assert_eq!(
            summary_line(&todos).as_deref(),
            Some("☐ Run the tests (1/2)")
        );
    }

    #[test]
    fn summary_is_absent_for_an_empty_plan() {
        assert_eq!(summary_line(&[]), None);
    }

    #[test]
    fn empty_list_is_valid() {
        assert!(validate(&[]).is_ok());
        assert!(validate(&[item("Run the tests", TodoStatus::InProgress)]).is_ok());
    }
}
