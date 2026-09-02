//! Session, API-key and ban lookups resolve on the primary even when the read
//! pool is unusable; listings may still use the replica.

use std::sync::Arc;

use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_users::{BannedIpRepository, UserRepository};
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
async fn auth_lookups_read_the_primary_but_listings_do_not() {
    let Some(db) = split_pool().await else {
        return;
    };
    let users = UserRepository::new(&db).expect("user repo");
    let bans = BannedIpRepository::new(&db).expect("ban repo");
    let nonce = Uuid::new_v4().simple().to_string();

    assert!(
        !users
            .session_exists(&SessionId::new(format!("sess-{nonce}")))
            .await
            .expect("session lookup on primary")
    );
    assert!(
        users
            .find_active_api_key_by_prefix(&format!("sp_{nonce}"))
            .await
            .expect("api key lookup on primary")
            .is_none()
    );
    assert!(!bans.is_banned("203.0.113.9").await.expect("ban lookup on primary"));

    assert!(
        users
            .list_sessions(&UserId::new(format!("user-{nonce}")))
            .await
            .is_err(),
        "listings stay on the read pool, which is closed in this fixture"
    );
}
