//! MCP registry route returns the enabled MCP servers as JSON.

use systemprompt_api::routes::mcp_registry_router;
use tower::ServiceExt;

use super::common::{body_to_string, empty_get, setup_ctx};

#[tokio::test]
async fn mcp_registry_returns_collection_payload() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = mcp_registry_router(&ctx);

    let resp = app.oneshot(empty_get("/")).await?;
    let (status, body) = body_to_string(resp).await?;
    assert!(status.is_success() || status.is_server_error(), "{status}");
    let parsed: serde_json::Value = serde_json::from_str(&body)?;
    assert!(parsed.is_object(), "registry payload must be a JSON object");
    Ok(())
}
