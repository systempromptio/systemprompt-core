//! Security-critical lookups must resolve on the primary even when the read
//! pool is unusable, so a regional replica that lags the primary can never
//! hide a fresh token, session or revocation.

use std::sync::Arc;

use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{AccessTokenId, AuthorizationCode, RefreshTokenId};
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
use uuid::Uuid;

async fn split_pool() -> DbPool {
    ensure_test_bootstrap();
    let live = test_db_pool().await;
    let write = live.write_pool();
    let dead = sqlx::PgPool::connect_lazy("postgres://closed:closed@127.0.0.1:1/closed")
        .expect("lazy pool");
    dead.close().await;
    Arc::new(Database::from_pools(Arc::new(dead), Some(write)))
}

#[tokio::test]
async fn token_and_revocation_lookups_read_the_primary() {
    let db = split_pool().await;
    let repo = OAuthRepository::new(&db);
    let nonce = Uuid::new_v4().simple().to_string();

    assert!(
        !repo
            .is_jti_revoked(&AccessTokenId::new(&nonce))
            .await
            .expect("jti lookup on primary")
    );
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
