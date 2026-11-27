//! The todo tool: the model keeps a session's task list by replacing it whole.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use harness_ids::SessionId;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::contract::{Tool, ToolContext, ToolOutput};
use crate::error::ToolError;
use crate::repair::parse_input;

/// One entry of a session's todo list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub status: TodoStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<TodoPriority>,
}

/// Where a todo stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

/// How much a todo matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TodoPriority {
    High,
    Medium,
    Low,
}

/// Stores a session's todo list, held by the todo tool as `dyn TodoSink`.
/// Implemented by the engine on top of its store; `replace` swaps the whole
/// list for a session.
#[async_trait]
pub trait TodoSink: Send + Sync {
    async fn replace(&self, session_id: &SessionId, todos: Vec<TodoItem>) -> Result<(), ToolError>;
}

/// Replaces the session's todo list.
#[derive(Clone)]
pub struct TodoTool {
    sink: Arc<dyn TodoSink>,
}

impl fmt::Debug for TodoTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TodoTool").finish_non_exhaustive()
    }
}

impl TodoTool {
    pub const fn new(sink: Arc<dyn TodoSink>) -> Self {
        Self { sink }
    }
}

#[derive(Debug, Deserialize)]
struct Input {
    todos: Vec<TodoItem>,
}

fn render(todos: &[TodoItem]) -> String {
    if todos.is_empty() {
        return "The todo list is empty.".to_owned();
    }
    todos
        .iter()
        .map(|todo| {
            let mark = match todo.status {
                TodoStatus::Pending => "[ ]",
                TodoStatus::InProgress => "[~]",
                TodoStatus::Completed => "[x]",
                TodoStatus::Cancelled => "[-]",
            };
            match todo.priority {
                Some(TodoPriority::High) => format!("{mark} {} (high)", todo.content),
                _ => format!("{mark} {}", todo.content),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        crate::names::TODO
    }

    fn description(&self) -> String {
        "Keep a todo list for the current task, visible to the user as you work. Each call replaces the whole list, \
         so always send every item with its current status. Use it for work with three or more steps or when the \
         user gives several things to do: write the plan up front, mark one item in_progress before starting it, and \
         mark it completed as soon as it is done rather than in batches. Use cancelled for items that no longer \
         apply. Skip it for single, trivial requests."
            .to_owned()
    }

    // JSON: the tool's input schema, sent to the model as JSON Schema.
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "The complete, updated todo list.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": {"type": "string", "description": "What needs doing, in a few words."},
                            "status": {"type": "string", "enum": ["pending", "in_progress", "completed", "cancelled"]},
                            "priority": {"type": "string", "enum": ["high", "medium", "low"]}
                        },
                        "required": ["content", "status"]
                    }
                }
            },
            "required": ["todos"]
        })
    }

    // JSON: the object the model wrote, checked against the schema above.
    async fn run(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let input: Input = parse_input(&self.schema(), input)?;
        if input
            .todos
            .iter()
            .any(|todo| todo.content.trim().is_empty())
        {
            return Err(ToolError::invalid("every todo needs non-empty content"));
        }
        self.sink
            .replace(&ctx.session_id, input.todos.clone())
            .await?;
        let open = input
            .todos
            .iter()
            .filter(|todo| matches!(todo.status, TodoStatus::Pending | TodoStatus::InProgress))
            .count();
        let title = format!("{open} open todo{}", if open == 1 { "" } else { "s" });
        Ok(ToolOutput::new(title, render(&input.todos))
            .with_metadata(json!({ "todos": input.todos })))
    }
}
