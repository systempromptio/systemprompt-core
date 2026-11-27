use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use harness_ids::SessionId;
use harness_tools::{TodoItem, TodoSink, TodoStatus, TodoTool, Tool, ToolError};
use serde_json::json;

use crate::support::{invalid_message, recorded};

#[derive(Debug, Default)]
struct Store(Mutex<Vec<(SessionId, Vec<TodoItem>)>>);

#[async_trait]
impl TodoSink for Store {
    async fn replace(&self, session_id: &SessionId, todos: Vec<TodoItem>) -> Result<(), ToolError> {
        self.0.lock().unwrap().push((session_id.clone(), todos));
        Ok(())
    }
}

#[tokio::test]
async fn replaces_the_list_and_renders_status_marks() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let store = Arc::new(Store::default());
    let tool = TodoTool::new(Arc::<Store>::clone(&store));
    let todos = json!([
        {"content": "plan", "status": "completed"},
        {"content": "build", "status": "in_progress", "priority": "high"},
        {"content": "ship", "status": "pending", "priority": "low"}
    ]);
    let out = tool.run(json!({"todos": todos}), &ctx).await.unwrap();
    assert_eq!(out.output, "[x] plan\n[~] build (high)\n[ ] ship");
    assert_eq!(out.title, "2 open todos");
    let saved = store.0.lock().unwrap().clone();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].0, ctx.session_id);
    assert_eq!(saved[0].1[1].status, TodoStatus::InProgress);
}

#[tokio::test]
async fn empty_content_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = recorded(dir.path());
    let tool = TodoTool::new(Arc::new(Store::default()));
    let input = json!({"todos": [{"content": "  ", "status": "pending"}]});
    let err = tool.run(input, &ctx).await.unwrap_err();
    assert!(invalid_message(&err).contains("non-empty content"));
}

#[test]
fn statuses_use_snake_case() {
    let item: TodoItem =
        serde_json::from_value(json!({"content": "a", "status": "in_progress"})).unwrap();
    assert_eq!(item.status, TodoStatus::InProgress);
}
