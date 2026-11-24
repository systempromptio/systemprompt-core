//! Users / sessions route.

use axum::routing::post;
use axum::{Extension, Router};
use systemprompt_identity_entry::http::users::sessions::revoke_all_mine;
use tower::ServiceExt;

use super::common::{json_post, request_context, setup_ctx};

#[tokio::test]
async fn revoke_all_mine_runs() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app: Router = Router::new()
        .route("/sessions/revoke-all", post(revoke_all_mine))
        .with_state(systemprompt_traits::AppContext::session_provider(&*ctx).expect("sessions"))
        .layer(Extension(request_context("user_sessions")));

    let resp = app
        .oneshot(json_post("/sessions/revoke-all", serde_json::json!({})))
        .await?;
    assert!(resp.status().as_u16() >= 200, "{}", resp.status());
    Ok(())
}
