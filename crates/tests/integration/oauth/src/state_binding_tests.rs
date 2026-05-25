//! Integration tests for OAuth state-binding (C1).

use crate::setup_test_db;
use chrono::{Duration, Utc};
use systemprompt_oauth::repository::{OAuthRepository, StateBindingParams};
use uuid::Uuid;

fn unique_token() -> String {
    format!("state_{}", Uuid::new_v4().simple())
}

#[tokio::test]
async fn roundtrip_consumes_once() {
    let db = setup_test_db().await;
    let repo = OAuthRepository::new(&db).expect("repo");
    let token = unique_token();

    repo.store_state_binding(
        StateBindingParams::builder(&token)
            .with_return_to("/dashboard")
            .with_client_id("client_state_test")
            .with_redirect_uri("https://example.invalid/cb")
            .build(),
    )
    .await
    .expect("store");

    let first = repo
        .consume_state_binding(&token)
        .await
        .expect("consume ok");
    assert!(first.is_some(), "first consume should succeed");
    assert_eq!(first.unwrap().return_to, "/dashboard");

    let second = repo
        .consume_state_binding(&token)
        .await
        .expect("second consume ok");
    assert!(
        second.is_none(),
        "single-use: second consume must return None"
    );
}

#[tokio::test]
async fn expired_row_rejected() {
    let db = setup_test_db().await;
    let repo = OAuthRepository::new(&db).expect("repo");
    let token = unique_token();

    repo.store_state_binding(
        StateBindingParams::builder(&token)
            .with_return_to("/x")
            .with_client_id("client_state_test_expired")
            .with_redirect_uri("https://example.invalid/cb")
            .with_expires_at(Utc::now() - Duration::seconds(1))
            .build(),
    )
    .await
    .expect("store");

    let consumed = repo
        .consume_state_binding(&token)
        .await
        .expect("consume ok");
    assert!(consumed.is_none(), "expired row must not consume");
}

#[tokio::test]
async fn tampered_state_rejected() {
    let db = setup_test_db().await;
    let repo = OAuthRepository::new(&db).expect("repo");
    let token = unique_token();

    repo.store_state_binding(
        StateBindingParams::builder(&token)
            .with_return_to("/orig")
            .with_client_id("client_state_test_tamper")
            .with_redirect_uri("https://example.invalid/cb")
            .build(),
    )
    .await
    .expect("store");

    let consumed = repo
        .consume_state_binding(&format!("{token}_tampered"))
        .await
        .expect("consume ok");
    assert!(consumed.is_none(), "lookup by a different token must miss");
}

#[tokio::test]
async fn cleanup_expired_removes_only_expired() {
    let db = setup_test_db().await;
    let repo = OAuthRepository::new(&db).expect("repo");
    let live = unique_token();
    let dead = unique_token();

    repo.store_state_binding(
        StateBindingParams::builder(&live)
            .with_return_to("/")
            .with_client_id("cleanup_live")
            .with_redirect_uri("https://example.invalid/cb")
            .build(),
    )
    .await
    .expect("store live");
    repo.store_state_binding(
        StateBindingParams::builder(&dead)
            .with_return_to("/")
            .with_client_id("cleanup_dead")
            .with_redirect_uri("https://example.invalid/cb")
            .with_expires_at(Utc::now() - Duration::seconds(60))
            .build(),
    )
    .await
    .expect("store dead");

    let removed = repo
        .cleanup_expired_state_bindings()
        .await
        .expect("cleanup ok");
    assert!(removed >= 1, "cleanup must reap at least the dead row");

    let live_consumed = repo
        .consume_state_binding(&live)
        .await
        .expect("consume live");
    assert!(live_consumed.is_some(), "live binding must survive cleanup");
}
