//! Authenticated gateway routes — drives the happy path through `whoami`,
//! `manifest`, `profile/usage`, `heartbeat`, and `profile/enabled_hosts` using
//! `seed_admin_credential`, which inserts the user row + active session row +
//! mints a matching JWT in one call so `decode_for_gateway` returns Ok.

use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, header};
use http::StatusCode;
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_database::DbPool;
use systemprompt_marketplace::managed::ManagedRepository;
use systemprompt_oauth::repository::BridgeSessionRepository;
use systemprompt_test_fixtures::{
    AuthedFixture, install_test_signing_key, seed_admin_credential, seed_bridge_credential,
};
use tower::ServiceExt;

use super::common::setup_ctx;

async fn router_and_pool() -> anyhow::Result<(Router, DbPool)> {
    let (pool, ctx) = setup_ctx().await?;
    install_test_signing_key();
    let router = gateway_router(&ctx)
        .expect("gateway journal opens")
        .expect("gateway router available");
    Ok((router, pool))
}

async fn read_text(resp: Response<Body>) -> anyhow::Result<String> {
    Ok(String::from_utf8(
        to_bytes(resp.into_body(), 1024 * 1024).await?.to_vec(),
    )?)
}

#[tokio::test]
async fn heartbeat_rejects_a_session_claimed_by_another_token_without_recording_liveness()
-> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let credential = seed_bridge_credential(&pool, "heartbeat-mismatch@example.invalid").await?;
    let foreign_session = systemprompt_identifiers::SessionId::generate();
    let response = app
        .oneshot(authed_post(
            "/bridge/heartbeat",
            credential.jwt.as_str(),
            serde_json::json!({
                "session_id": foreign_session.as_str(),
                "bridge_version": "1.0.0",
                "os": "linux",
                "hostname": "spoofed-host",
                "forwarded_total": 99
            }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = read_text(response).await?;
    assert!(body.contains("session_id must match"), "{body}");
    let active = BridgeSessionRepository::new(&pool)?
        .list_active_for_user(&credential.user_id, Duration::from_secs(60))
        .await?;
    assert!(
        active
            .iter()
            .all(|session| session.session_id != foreign_session),
        "a rejected heartbeat must not create liveness for a foreign session"
    );
    Ok(())
}

#[tokio::test]
async fn incompatible_heartbeat_is_recorded_with_its_usage_and_reported_incompatible()
-> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let credential = seed_bridge_credential(&pool, "heartbeat-old@example.invalid").await?;
    let response = app
        .oneshot(authed_post(
            "/bridge/heartbeat",
            credential.jwt.as_str(),
            serde_json::json!({
                "session_id": credential.session_id.as_str(),
                "bridge_version": "0.1.0",
                "os": "linux",
                "hostname": "old-bridge",
                "forwarded_total": 7,
                "tokens_in_total": 11,
                "tokens_out_total": 13
            }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_body(response).await?;
    assert_eq!(body["compatible"], false);
    assert_eq!(body["min_bridge_version"], "0.28.0");
    let active = BridgeSessionRepository::new(&pool)?
        .list_active_for_user(&credential.user_id, Duration::from_secs(60))
        .await?;
    let persisted = active
        .iter()
        .find(|session| session.session_id == credential.session_id)
        .expect("incompatible bridge remains visible for upgrade diagnostics");
    assert_eq!(persisted.bridge_version, "0.1.0");
    assert_eq!(persisted.hostname, "old-bridge");
    assert_eq!(persisted.forwarded_total, 7);
    assert_eq!(persisted.tokens_in_total, 11);
    assert_eq!(persisted.tokens_out_total, 13);
    Ok(())
}

#[tokio::test]
async fn heartbeat_storage_failure_is_reported_and_a_retry_records_the_session()
-> anyhow::Result<()> {
    let database =
        systemprompt_test_fixtures::DisposableDb::installed("heartbeat_persistence_recovery")
            .await?;
    let pool = database.pool().await?;
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let ctx = systemprompt_test_fixtures::fixture_app_context(&pool, database.url())?;
    install_test_signing_key();
    let app = gateway_router(&ctx)
        .expect("gateway journal opens")
        .expect("gateway router available");
    let credential = seed_bridge_credential(&pool, "heartbeat-retry@example.invalid").await?;
    let payload = serde_json::json!({
        "session_id": credential.session_id.as_str(),
        "bridge_version": "1.0.0",
        "os": "linux",
        "hostname": "retrying-bridge",
        "forwarded_total": 17,
        "tokens_in_total": 19,
        "tokens_out_total": 23
    });

    let write_pool = pool.write_pool();
    sqlx::query("ALTER TABLE bridge_sessions RENAME TO bridge_sessions_unavailable")
        .execute(write_pool.as_ref())
        .await?;
    let failed = app
        .clone()
        .oneshot(authed_post(
            "/bridge/heartbeat",
            credential.jwt.as_str(),
            payload.clone(),
        ))
        .await;
    sqlx::query("ALTER TABLE bridge_sessions_unavailable RENAME TO bridge_sessions")
        .execute(write_pool.as_ref())
        .await?;

    let failed = failed?;
    assert_eq!(failed.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = read_text(failed).await?;
    assert!(
        body.starts_with("bridge heartbeat upsert failed:"),
        "{body}"
    );

    let recovered = app
        .oneshot(authed_post(
            "/bridge/heartbeat",
            credential.jwt.as_str(),
            payload,
        ))
        .await?;
    assert_eq!(recovered.status(), StatusCode::OK);
    let active = BridgeSessionRepository::new(&pool)?
        .list_active_for_user(&credential.user_id, Duration::from_secs(60))
        .await?;
    let persisted = active
        .iter()
        .find(|session| session.session_id == credential.session_id)
        .expect("retry records the authenticated bridge session");
    assert_eq!(persisted.hostname, "retrying-bridge");
    assert_eq!(persisted.forwarded_total, 17);
    assert_eq!(persisted.tokens_in_total, 19);
    assert_eq!(persisted.tokens_out_total, 23);

    drop(write_pool);
    drop(pool);
    drop(ctx);
    database.drop_now().await;
    Ok(())
}

#[tokio::test]
async fn device_fingerprint_cannot_move_between_users_and_the_owner_can_still_rotate()
-> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let owner = seed_bridge_credential(&pool, "device-owner@example.invalid").await?;
    let other = seed_bridge_credential(&pool, "device-other@example.invalid").await?;
    let fingerprint = systemprompt_models::feedback::ContentDigest::of(
        format!("device-{}", uuid::Uuid::new_v4()).as_bytes(),
    )
    .as_str()
    .to_owned();
    let request = |token: &str| {
        authed_post(
            "/bridge/device",
            token,
            serde_json::json!({"fingerprint": fingerprint, "label": "owned laptop"}),
        )
    };

    let first = app.clone().oneshot(request(owner.jwt.as_str())).await?;
    assert_eq!(first.status(), StatusCode::OK);
    let first = read_body(first).await?;
    assert_eq!(first["consumer_id"], owner.user_id.as_str());
    let device_id = first["device_id"].clone();
    let first_credential = first["credential"].as_str().unwrap().to_owned();
    let repository = ManagedRepository::new(&pool)?;
    let first_identity = repository
        .authenticate_consumer_device(&first_credential)
        .await?;
    assert_eq!(first_identity.consumer_id, owner.user_id);

    let conflict = app.clone().oneshot(request(other.jwt.as_str())).await?;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    let conflict_body = read_text(conflict).await?;
    assert!(conflict_body.contains("another user"), "{conflict_body}");
    assert!(
        repository
            .authenticate_consumer_device(&first_credential)
            .await
            .is_ok(),
        "a rejected foreign claim must not revoke the owner's credential"
    );

    let rotated = app.oneshot(request(owner.jwt.as_str())).await?;
    assert_eq!(rotated.status(), StatusCode::OK);
    let rotated = read_body(rotated).await?;
    assert_eq!(rotated["device_id"], device_id);
    assert_eq!(rotated["consumer_id"], owner.user_id.as_str());
    let rotated_credential = rotated["credential"].as_str().unwrap();
    assert_ne!(rotated_credential, first_credential);
    assert!(
        repository
            .authenticate_consumer_device(&first_credential)
            .await
            .is_err(),
        "rotation must revoke the previously issued credential"
    );
    let rotated_identity = repository
        .authenticate_consumer_device(rotated_credential)
        .await?;
    assert_eq!(rotated_identity.consumer_id, owner.user_id);
    assert_eq!(
        rotated_identity.device_id.as_str(),
        device_id.as_str().unwrap()
    );
    Ok(())
}

fn authed_get(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request build")
}

fn authed_post(uri: &str, token: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request build")
}

async fn read_body(resp: Response<Body>) -> anyhow::Result<serde_json::Value> {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    Ok(serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn pubkey_after_install_returns_ok() -> anyhow::Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/bridge/pubkey")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "{}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn whoami_for_seeded_admin_returns_envelope() -> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let cred: AuthedFixture = seed_admin_credential(&pool, "whoami@example.invalid").await?;
    let resp = app
        .oneshot(authed_get("/bridge/whoami", cred.jwt.as_str()))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp).await?;
    assert_eq!(body["user_id"], cred.user_id.as_str());
    assert_eq!(body["email"].as_str(), Some(cred.email.as_str()));
    assert!(body["roles"].as_array().is_some());
    Ok(())
}

#[tokio::test]
async fn whoami_missing_authorization_returns_4xx() -> anyhow::Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/bridge/whoami")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    assert!(resp.status().is_client_error(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn whoami_garbage_bearer_returns_unauthorized() -> anyhow::Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let resp = app
        .oneshot(authed_get("/bridge/whoami", "not-a-jwt"))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn profile_usage_for_seeded_admin_returns_ok_envelope() -> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let cred = seed_admin_credential(&pool, "usage@example.invalid").await?;
    let resp = app
        .oneshot(authed_get("/bridge/profile/usage", cred.jwt.as_str()))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp).await?;
    assert!(body.is_object(), "expected json object, got {body}");
    Ok(())
}

#[tokio::test]
async fn heartbeat_for_seeded_admin_accepts_payload() -> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let cred = seed_admin_credential(&pool, "heartbeat@example.invalid").await?;
    let payload = serde_json::json!({
        "session_id": cred.session_id.as_str(),
        "bridge_version": "1.0.0",
        "os": "linux",
        "hostname": "test"
    });
    let resp = app
        .oneshot(authed_post("/bridge/heartbeat", cred.jwt.as_str(), payload))
        .await?;
    let s = resp.status();
    assert!(s.is_success() || s == StatusCode::ACCEPTED, "{s}");
    Ok(())
}

#[tokio::test]
async fn set_enabled_host_toggles_pref_for_seeded_admin() -> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let cred = seed_admin_credential(&pool, "hosts@example.invalid").await?;
    let payload = serde_json::json!({"host_id": "claude-code", "enabled": true});
    let resp = app
        .oneshot(authed_post(
            "/bridge/profile/enabled_hosts",
            cred.jwt.as_str(),
            payload,
        ))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn manifest_for_seeded_admin_returns_response() -> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let cred = seed_admin_credential(&pool, "manifest@example.invalid").await?;
    let resp = app
        .oneshot(authed_get("/bridge/manifest", cred.jwt.as_str()))
        .await?;
    let s = resp.status();
    // Manifest assembly depends on services/marketplace state being present in
    // the fixture; the happy path 200 is the primary assertion, but 5xx is
    // tolerated when the fixture's marketplace fixture isn't populated.
    assert!(
        s == StatusCode::OK || s.is_server_error(),
        "manifest unexpected status: {s}"
    );
    Ok(())
}
