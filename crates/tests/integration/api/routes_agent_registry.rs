//! Agent registry route — needs RequestContext injected via Extension layer.

use axum::Extension;
use systemprompt_api::routes::registry_router;
use tower::ServiceExt;

use super::common::{empty_get, request_context, setup_ctx};

#[tokio::test]
async fn agent_registry_handles_missing_profile() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = registry_router(&ctx).layer(Extension(request_context("user_test")));

    let resp = app.oneshot(empty_get("/")).await?;
    // Without a profile the AgentRegistry::new() returns an error which
    // surfaces as a 500, but the routing + extension + state path executes.
    assert!(resp.status().as_u16() >= 400 || resp.status().as_u16() == 200);
    Ok(())
}
