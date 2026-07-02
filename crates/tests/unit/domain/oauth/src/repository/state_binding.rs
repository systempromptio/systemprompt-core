// DB-backed OAuth state-binding tests (store/consume single-use, expiry,
// cleanup) plus the builder defaults.

use chrono::{Duration, Utc};
use systemprompt_identifiers::ClientId;
use systemprompt_oauth::repository::{OAuthRepository, StateBindingParams};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn repo() -> Option<OAuthRepository> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    Some(OAuthRepository::new(&pool).expect("repo"))
}

#[test]
fn builder_applies_defaults() {
    let params = StateBindingParams::builder("tok").build();
    assert_eq!(params.return_to, "/");
    assert_eq!(params.client_id.as_str(), "");
    assert_eq!(params.redirect_uri, "");
    assert!(params.expires_at > Utc::now());
}

#[test]
fn builder_overrides_fields() {
    let exp = Utc::now() + Duration::minutes(5);
    let client_id = ClientId::new("client-x");
    let params = StateBindingParams::builder("tok")
        .with_return_to("/dashboard")
        .with_client_id(&client_id)
        .with_redirect_uri("https://app.invalid/cb")
        .with_expires_at(exp)
        .build();
    assert_eq!(params.return_to, "/dashboard");
    assert_eq!(params.client_id.as_str(), "client-x");
    assert_eq!(params.redirect_uri, "https://app.invalid/cb");
    assert_eq!(params.expires_at, exp);
}

#[tokio::test]
async fn store_then_consume_once() {
    let Some(repo) = repo().await else { return };
    let token = format!("state-{}", Uuid::new_v4());
    let client_id = ClientId::new("cid-x");
    repo.store_state_binding(
        StateBindingParams::builder(&token)
            .with_return_to("/back")
            .with_client_id(&client_id)
            .with_redirect_uri("https://app.invalid/cb")
            .build(),
    )
    .await
    .expect("store");

    let row = repo
        .consume_state_binding(&token)
        .await
        .expect("consume")
        .expect("present");
    assert_eq!(row.return_to, "/back");
    assert_eq!(row.client_id.as_str(), "cid-x");
    assert_eq!(row.redirect_uri, "https://app.invalid/cb");

    // Second consume yields None (single-use).
    assert!(
        repo.consume_state_binding(&token)
            .await
            .expect("consume again")
            .is_none()
    );
}

#[tokio::test]
async fn consume_unknown_returns_none() {
    let Some(repo) = repo().await else { return };
    assert!(
        repo.consume_state_binding(&format!("nope-{}", Uuid::new_v4()))
            .await
            .expect("consume")
            .is_none()
    );
}

#[tokio::test]
async fn expired_binding_cannot_be_consumed() {
    let Some(repo) = repo().await else { return };
    let token = format!("state-{}", Uuid::new_v4());
    repo.store_state_binding(
        StateBindingParams::builder(&token)
            .with_expires_at(Utc::now() - Duration::minutes(1))
            .build(),
    )
    .await
    .expect("store");
    assert!(
        repo.consume_state_binding(&token)
            .await
            .expect("consume")
            .is_none()
    );
}

#[tokio::test]
async fn cleanup_expired_state_bindings_removes_past() {
    let Some(repo) = repo().await else { return };
    let token = format!("state-{}", Uuid::new_v4());
    repo.store_state_binding(
        StateBindingParams::builder(&token)
            .with_expires_at(Utc::now() - Duration::hours(1))
            .build(),
    )
    .await
    .expect("store");
    let removed = repo
        .cleanup_expired_state_bindings()
        .await
        .expect("cleanup");
    assert!(removed >= 1);
}
