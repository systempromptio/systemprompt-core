use std::sync::Arc;
use std::time::Duration;

use systemprompt_database::{CoordinationRepositories, Database, DbPool, PoolConfig};
use systemprompt_identifiers::{InstanceId, LeaseKey};
use systemprompt_test_fixtures::{test_database_url, unique_name};
use systemprompt_traits::{CoordinationError, LeaseRepository};

async fn db_pool() -> DbPool {
    let cfg = PoolConfig {
        max_connections: 4,
        min_connections: 0,
        acquire_timeout: Duration::from_secs(30),
        idle_timeout: Duration::from_secs(30),
        max_lifetime: Duration::from_secs(300),
        statement_cache_capacity: 100,
    };
    Arc::new(
        Database::connect(&test_database_url(), None, &cfg)
            .await
            .expect("database"),
    )
}

#[tokio::test]
async fn a_holder_that_lost_its_lease_does_not_release_the_new_holders_row() {
    let db = db_pool().await;
    let stale = CoordinationRepositories::new(&db, InstanceId::new(unique_name("node")));
    let successor = CoordinationRepositories::new(&db, InstanceId::new(unique_name("node")));
    let key = LeaseKey::new(unique_name("lease"));
    let lost = stale
        .leases
        .claim(&key, Duration::from_secs(60))
        .await
        .expect("claim")
        .expect("first claim");
    sqlx::query(
        "UPDATE instance_leases SET expires_at = CURRENT_TIMESTAMP - make_interval(secs => 1) \
         WHERE lease_key = $1",
    )
    .bind(key.as_str())
    .execute(&*db.write_pool())
    .await
    .expect("expire lease");
    let taken = successor
        .leases
        .claim(&key, Duration::from_secs(60))
        .await
        .expect("claim")
        .expect("successor takes the expired lease");

    let renewed = stale.leases.renew(&lost, Duration::from_secs(60)).await;
    assert!(matches!(renewed, Err(CoordinationError::Fenced { .. })));
    let released = stale.leases.release(&lost).await;
    assert!(
        matches!(released, Err(CoordinationError::Fenced { .. })),
        "the stale release is fenced: {released:?}"
    );
    let holder = successor
        .leases
        .holder(&key)
        .await
        .expect("holder")
        .expect("still held");
    assert_eq!(holder, taken);

    successor
        .leases
        .release(&taken)
        .await
        .expect("the current holder releases its own lease");
    assert!(
        successor
            .leases
            .holder(&key)
            .await
            .expect("holder")
            .is_none()
    );
}
