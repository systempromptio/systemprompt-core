//! Tests for [`McpToolExecutor`] constructor and helpers.
//!
//! Building a full executor end-to-end requires repos and a live handler
//! implementation. We exercise construction + the inherent value-type
//! surface to give the file at least one passing branch.

use std::sync::Arc;
use systemprompt_ai::repository::AiRequestRepository;
use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_mcp::services::ui_renderer::UiRendererRegistry;
use systemprompt_mcp::{ArtifactIngest, McpToolExecutor};
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::DynToolCallIntentClaims;

fn intents(db: &systemprompt_database::DbPool) -> DynToolCallIntentClaims {
    Arc::new(AiRequestRepository::new(db))
}

#[tokio::test]
async fn tool_executor_construction_and_clone() {
    let db = test_db_pool().await;
    let tool_repo = Arc::new(ToolUsageRepository::new(&db));
    let art_repo = Arc::new(ArtifactIngest::from_db(
        &db,
        systemprompt_governance::policy::secrets::SecretScannerCell::default(),
    ));
    let exec = McpToolExecutor::new(
        tool_repo,
        Some(intents(&db)),
        art_repo,
        Arc::new(UiRendererRegistry::with_builtins()),
        systemprompt_mcp::ServerCallers {
            server: McpServerId::new("srv-x"),
            verification: systemprompt_test_fixtures::fixture_caller_verification(),
        },
    );
    let _ = exec.clone();
    let _ = format!("{exec:?}");
}
