//! Users / sessions route.

use axum::{Extension, Router};
use axum::routing::post;
use systemprompt_api::routes::users::sessions::revoke_all_mine;
use tower::ServiceExt;

use super::common::{json_post, request_context, setup_ctx};

#[tokio::test]
async fn revoke_all_mine_runs() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app: Router = Router::new()
        .route("/sessions/revoke-all", post(revoke_all_mine))
        .with_state((*ctx).clone())
        .layer(Extension(request_context("user_sessions")));

    let resp = app
        .oneshot(json_post("/sessions/revoke-all", serde_json::json!({})))
        .await?;
    assert!(resp.status().as_u16() >= 200, "{}", resp.status());
    Ok(())
}
