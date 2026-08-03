//! Admin API-key routes driven against a real, existing user row.
//!
//! The pre-existing suite issues keys for a user that was never inserted, so
//! every write fails the users foreign key and the handlers' success bodies
//! (the issued-key response, the `ApiKeyView` projection, the revoke result)
//! never run. Seeding the user first puts issue → list → revoke on their real
//! paths, which is also the only way to reach the revoke-miss 404.

use axum::{Extension, Router};
use http_body_util::BodyExt;
use systemprompt_api::routes::admin;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::seed_user_row;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{empty_delete, empty_get, json_post, request_context, setup_ctx};

fn app(ctx: &AppContext, user: &UserId) -> Router {
    admin::router()
        .with_state(ctx.clone())
        .layer(Extension(request_context(user.as_str())))
}

async fn json_body(
    resp: axum::http::Response<axum::body::Body>,
) -> anyhow::Result<serde_json::Value> {
    let status = resp.status();
    let bytes = resp.into_body().collect().await?.to_bytes();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok(serde_json::from_str(&text)
        .unwrap_or_else(|_| serde_json::json!({"_raw": text, "_status": status.as_u16()})))
}

async fn seeded_user() -> anyhow::Result<(UserId, std::sync::Arc<AppContext>)> {
    let (pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("keyholder-{}", Uuid::new_v4().simple()));
    seed_user_row(&pool, &user, &format!("{}@keys.invalid", user.as_str())).await?;
    Ok((user, ctx))
}

#[tokio::test]
async fn issuing_a_key_returns_the_secret_exactly_once() -> anyhow::Result<()> {
    let (user, ctx) = seeded_user().await?;
    let name = format!("ci-{}", Uuid::new_v4().simple());

    let resp = app(&ctx, &user)
        .oneshot(json_post("/api-keys", serde_json::json!({ "name": name })))
        .await?;
    assert_eq!(resp.status().as_u16(), 201, "{:?}", resp.status());
    let issued = json_body(resp).await?;

    let secret = issued["secret"].as_str().unwrap_or_default();
    assert!(
        !secret.is_empty(),
        "the plaintext secret is returned: {issued}"
    );
    assert_eq!(issued["name"].as_str(), Some(name.as_str()), "{issued}");
    let prefix = issued["key_prefix"].as_str().unwrap_or_default();
    assert!(
        !prefix.is_empty(),
        "the stored prefix identifies the key: {issued}"
    );

    // The listing is the only later view of the key, and it must not be able to
    // hand the secret back out.
    let listed = json_body(app(&ctx, &user).oneshot(empty_get("/api-keys")).await?).await?;
    let entries = listed.as_array().cloned().unwrap_or_default();
    assert_eq!(
        entries.len(),
        1,
        "exactly the issued key is listed: {listed}"
    );
    assert_eq!(entries[0]["key_prefix"].as_str(), Some(prefix), "{listed}");
    assert!(
        entries[0].get("secret").is_none(),
        "a listed key must never carry the plaintext secret: {listed}"
    );
    assert!(
        entries[0]["revoked_at"].is_null(),
        "a fresh key is not revoked: {listed}"
    );
    Ok(())
}

#[tokio::test]
async fn a_key_is_revoked_once_and_the_second_attempt_is_a_404() -> anyhow::Result<()> {
    let (user, ctx) = seeded_user().await?;
    let issued = json_body(
        app(&ctx, &user)
            .oneshot(json_post(
                "/api-keys",
                serde_json::json!({ "name": "revoke-me" }),
            ))
            .await?,
    )
    .await?;
    let id = issued["id"]
        .as_str()
        .expect("the issued key carries its id")
        .to_owned();

    let first = app(&ctx, &user)
        .oneshot(empty_delete(&format!("/api-keys/{id}")))
        .await?;
    assert_eq!(first.status().as_u16(), 204, "{:?}", first.status());

    let second = app(&ctx, &user)
        .oneshot(empty_delete(&format!("/api-keys/{id}")))
        .await?;
    assert_eq!(
        second.status().as_u16(),
        404,
        "revoking an already-revoked key must not report success"
    );
    Ok(())
}

#[tokio::test]
async fn one_user_cannot_revoke_another_users_key() -> anyhow::Result<()> {
    let (owner, ctx) = seeded_user().await?;
    let (attacker, _ctx2) = seeded_user().await?;
    let issued = json_body(
        app(&ctx, &owner)
            .oneshot(json_post(
                "/api-keys",
                serde_json::json!({ "name": "owned" }),
            ))
            .await?,
    )
    .await?;
    let id = issued["id"].as_str().expect("issued id").to_owned();

    let resp = app(&ctx, &attacker)
        .oneshot(empty_delete(&format!("/api-keys/{id}")))
        .await?;

    assert_eq!(
        resp.status().as_u16(),
        404,
        "a key is scoped to its owner; another user must not be able to revoke it"
    );
    let still_listed = json_body(app(&ctx, &owner).oneshot(empty_get("/api-keys")).await?).await?;
    assert!(
        still_listed[0]["revoked_at"].is_null(),
        "the owner's key must survive the foreign revoke attempt: {still_listed}"
    );
    Ok(())
}

#[tokio::test]
async fn a_users_listing_shows_only_their_own_keys() -> anyhow::Result<()> {
    let (owner, ctx) = seeded_user().await?;
    let (other, _) = seeded_user().await?;
    app(&ctx, &owner)
        .oneshot(json_post(
            "/api-keys",
            serde_json::json!({ "name": "mine" }),
        ))
        .await?;

    let theirs = json_body(app(&ctx, &other).oneshot(empty_get("/api-keys")).await?).await?;

    assert_eq!(
        theirs.as_array().map(Vec::len),
        Some(0),
        "a freshly seeded user sees no keys belonging to anyone else: {theirs}"
    );
    Ok(())
}
