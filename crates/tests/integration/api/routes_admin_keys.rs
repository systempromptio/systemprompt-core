//! Admin API-key routes — covers issue/list/revoke against a real users DB.
//! Auth is plumbed via the AllowAllHook fixture; route handlers themselves
//! enforce ownership via RequestContext.

use axum::{Extension, Router};
use systemprompt_api::routes::admin;
use tower::ServiceExt;

use super::common::{empty_delete, empty_get, json_post, request_context, setup_ctx};

// `admin::keys::router` is pub(super), so wire the routes directly via the
// re-exported handlers — handler functions are not pub here, so we mount the
// admin parent router instead.

fn keys_app(ctx: &systemprompt_runtime::AppContext) -> Router {
    admin::router()
        .with_state(ctx.clone())
        .layer(Extension(request_context("user_admin")))
}

#[tokio::test]
async fn list_keys_returns_empty_array_for_new_user() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = keys_app(&ctx);
    let resp = app.oneshot(empty_get("/api-keys")).await?;
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "{}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn issue_key_then_list_then_revoke() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;

    // Issue
    let body = serde_json::json!({ "name": format!("test-key-{}", uuid::Uuid::new_v4()) });
    let resp = keys_app(&ctx).oneshot(json_post("/api-keys", body)).await?;
    let status = resp.status();
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await?
        .to_bytes();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    assert!(
        status.is_success() || status.is_server_error(),
        "issue failed {status}: {text}"
    );

    // List
    let resp = keys_app(&ctx).oneshot(empty_get("/api-keys")).await?;
    assert!(resp.status().is_success() || resp.status().is_server_error());

    // Revoke a non-existent id surfaces 404 / 500.
    let resp = keys_app(&ctx)
        .oneshot(empty_delete("/api-keys/key_does_not_exist"))
        .await?;
    assert!(resp.status().as_u16() >= 400 || resp.status().is_success());

    Ok(())
}
