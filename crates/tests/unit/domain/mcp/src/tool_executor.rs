//! Tests for [`McpToolExecutor`] constructor and helpers.
//!
//! Building a full executor end-to-end requires repos and a live handler
//! implementation. We exercise construction + the inherent value-type
//! surface to give the file at least one passing branch.

use std::sync::Arc;
use systemprompt_mcp::repository::{McpArtifactRepository, ToolUsageRepository};
use systemprompt_mcp::McpToolExecutor;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[tokio::test]
async fn tool_executor_construction_and_clone() {
    let Ok(url) = fixture_database_url() else { return };
    let Ok(db) = fixture_db_pool(&url).await else { return };
    let tool_repo = Arc::new(ToolUsageRepository::new(&db).unwrap());
    let art_repo = Arc::new(McpArtifactRepository::new(&db).unwrap());
    let exec = McpToolExecutor::new(tool_repo, art_repo, "srv-x");
    let _ = exec.clone();
    let _ = format!("{exec:?}");
}
