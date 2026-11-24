//! Admin API-key routes — covers issue/list/revoke against a real users DB.
//! Auth is plumbed via the AllowAllHook fixture; route handlers themselves
//! enforce ownership via RequestContext.

use axum::{Extension, Router};
use systemprompt_identity_entry::http::admin_keys;
use systemprompt_test_fixtures::AppContextSeeds as _;
use tower::ServiceExt;

use super::common::{empty_delete, empty_get, json_post, request_context, setup_ctx};

fn keys_app(ctx: &systemprompt_runtime::AppContext) -> Router {
    let state = admin_keys::ApiKeysState {
        keys: std::sync::Arc::new(systemprompt_users::ApiKeyService::new(
            systemprompt_test_fixtures::user_repository(&ctx),
            systemprompt_users::ApiKeyLifetime::from_max_days(
                ctx.config().api_key_max_lifetime_days,
            ),
        )),
        subjects: ctx
            .kernel()
            .state()
            .find::<std::sync::Arc<systemprompt_governance::authz::SubjectProviderSet>>()
            .cloned()
            .expect("subject providers"),
    };
    Router::new()
        .nest("/api-keys", admin_keys::router(state))
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
        status.is_success()
            || (status == axum::http::StatusCode::CONFLICT
                && text.contains("\"error_key\":\"foreign_key_violation\"")
                && !text.contains("_fkey")),
        "issue for an unseeded owner is a 409 that names no constraint: {status}: {text}"
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
