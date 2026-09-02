//! Security-critical lookups must resolve on the primary even when the read
//! pool is unusable, so a regional replica that lags the primary can never
//! hide a fresh token, session or revocation.

use std::sync::Arc;

use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{AuthorizationCode, RefreshTokenId};
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn split_pool() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let live = fixture_db_pool(&url).await.ok()?;
    let write = live.write_pool_arc().ok()?;
    let dead = sqlx::PgPool::connect_lazy("postgres://closed:closed@127.0.0.1:1/closed").ok()?;
    dead.close().await;
    Some(Arc::new(Database::from_pools(Arc::new(dead), Some(write))))
}

#[tokio::test]
async fn token_and_revocation_lookups_read_the_primary() {
    let Some(db) = split_pool().await else {
        return;
    };
    let repo = OAuthRepository::new(&db).expect("repo");
    let nonce = Uuid::new_v4().simple().to_string();

    assert!(!repo.is_jti_revoked(&nonce).await.expect("jti lookup on primary"));
    repo.validate_setup_token(&nonce)
        .await
        .expect("setup token lookup on primary");
    assert!(
        repo.find_client_id_from_auth_code(&AuthorizationCode::new(&nonce))
            .await
            .expect("auth code lookup on primary")
            .is_none()
    );
    assert!(
        repo.find_client_id_from_refresh_token(&RefreshTokenId::new(&nonce))
            .await
            .expect("refresh token lookup on primary")
            .is_none()
    );
}
