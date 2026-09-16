//! Session attestation reads the primary even when the read pool is unusable;
//! per-user listings may still use the replica.

use std::sync::Arc;

use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use systemprompt_users::SessionRepository;
use uuid::Uuid;

async fn split_pool_or_skip() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let live = fixture_db_pool(&url).await.ok()?;
    let write = live.write_pool_arc().ok()?;
    let dead = sqlx::PgPool::connect_lazy("postgres://closed:closed@127.0.0.1:1/closed").ok()?;
    dead.close().await;
    Some(Arc::new(Database::from_pools(Arc::new(dead), Some(write))))
}

#[tokio::test]
async fn attestation_lookup_reads_the_primary_but_listing_does_not() {
    let Some(db) = split_pool_or_skip().await else {
        return;
    };
    let repo = SessionRepository::new(&db).expect("repo");
    let nonce = Uuid::new_v4().simple().to_string();
    let session_id = SessionId::new(format!("sess-{nonce}"));

    assert!(
        repo.find_active_by_id(&session_id)
            .await
            .expect("attestation lookup on primary")
            .is_none()
    );
    assert!(
        repo.list_active_by_user(&UserId::new(format!("user-{nonce}")))
            .await
            .is_err(),
        "listings stay on the read pool, which is closed in this fixture"
    );
    assert_eq!(
        repo.count_sessions_by_fingerprint(&nonce, 24)
            .await
            .expect("primary count"),
        0
    );
    assert_eq!(
        repo.count_unique_ips_by_fingerprint(&nonce, 7)
            .await
            .expect("primary IP count"),
        0
    );
    assert!(
        repo.find_recent_by_fingerprint(&nonce, 60)
            .await
            .expect("primary reuse lookup")
            .is_none()
    );
    assert!(
        repo.get_session_for_behavioral_analysis(&session_id)
            .await
            .expect("primary behavior lookup")
            .is_none()
    );
    assert!(
        repo.get_session_starts_by_fingerprint(&nonce, 7)
            .await
            .expect("primary starts")
            .is_empty()
    );
    assert_eq!(
        repo.get_session_velocity(&session_id)
            .await
            .expect("primary velocity"),
        (None, None)
    );
    assert_eq!(
        repo.count_active_fingerprint(&nonce)
            .await
            .expect("primary active count"),
        0
    );
    assert!(
        repo.find_reusable_fingerprint(&nonce)
            .await
            .expect("primary reusable session")
            .is_none()
    );
}
