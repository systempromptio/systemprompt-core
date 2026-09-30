//! Tests for [`McpToolExecutor`] constructor and helpers.
//!
//! Building a full executor end-to-end requires repos and a live handler
//! implementation. We exercise construction + the inherent value-type
//! surface to give the file at least one passing branch.

use std::sync::Arc;
use systemprompt_ai::repository::AiRequestRepository;
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_mcp::{ArtifactIngest, McpToolExecutor};
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::DynToolCallIntentClaims;

fn intents(db: &systemprompt_database::DbPool) -> DynToolCallIntentClaims {
    Arc::new(AiRequestRepository::new(db).unwrap())
}

#[tokio::test]
async fn tool_executor_construction_and_clone() {
    let db = test_db_pool().await;
    let tool_repo = Arc::new(ToolUsageRepository::new(&db).unwrap());
    let art_repo = Arc::new(ArtifactIngest::from_db(&db, None).unwrap());
    let exec = McpToolExecutor::new(tool_repo, intents(&db), art_repo, "srv-x");
    let _ = exec.clone();
    let _ = format!("{exec:?}");
}
