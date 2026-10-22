//! A committed config plane write reaches a node's `ConfigWatch` through the
//! `sp_bump_config_version()` trigger's `config:<plane>` notification.
//!
//! The poll interval is an hour, so only the notification can explain the
//! reload: the bridge listening on `systemprompt_cache` advances the
//! `AccessControl` epoch and the watch reloads from the database. The reload
//! reads the test's own row, so concurrent tests writing the same table cannot
//! satisfy it.

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use systemprompt_database::{Database, PgConfigVersion};
use systemprompt_events::{EventBroadcasters, PostgresEventBridge};
use systemprompt_test_fixtures::{test_database_url, unique_instance};
use systemprompt_traits::{
    BackgroundTasks, CacheDomain, CacheEpochs, ConfigVersionSource, ConfigWatch, WatchSignals,
};

use crate::setup_test_pool;

const BOUND: Duration = Duration::from_secs(10);

async fn row_present(pool: PgPool, id: String) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM access_control_entities \
         WHERE entity_type = 'plugin' AND entity_id = $1)",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
}

#[tokio::test]
async fn a_trigger_notification_reloads_the_watch_without_polling() {
    let pool = setup_test_pool().await;
    let epochs = CacheEpochs::new();
    let bridge = PostgresEventBridge::new(
        (*pool).clone(),
        unique_instance(),
        epochs.clone(),
        Arc::new(EventBroadcasters::new()),
    )
    .start();
    assert!(
        tokio::time::timeout(BOUND, bridge.listening())
            .await
            .expect("listen in time"),
        "relay stopped before listening"
    );

    let db = Arc::new(
        Database::new_postgres(&test_database_url())
            .await
            .expect("connect database"),
    );
    let version = PgConfigVersion::new(&db);
    let before = version.current_version().await.expect("read version");

    let id = format!("test_watch_{}", uuid::Uuid::new_v4().simple());
    let tasks = BackgroundTasks::new();
    let watch = ConfigWatch::new(false);
    let (reload_pool, reload_id) = ((*pool).clone(), id.clone());
    watch.spawn(
        &tasks,
        WatchSignals {
            domain: CacheDomain::AccessControl,
            epochs: epochs.clone(),
            version: Arc::new(version.clone()),
            poll: Duration::from_secs(3600),
        },
        move || row_present(reload_pool.clone(), reload_id.clone()),
    );

    sqlx::query(
        "INSERT INTO access_control_entities (entity_type, entity_id, source) \
         VALUES ('plugin', $1, 'bundle:test')",
    )
    .bind(&id)
    .execute(&*pool)
    .await
    .expect("insert entity");

    tokio::time::timeout(BOUND, async {
        while !*watch.get() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the notification reloads the watch");
    let after = version.current_version().await.expect("read version");
    assert!(after > before, "the write bumped config_version");

    sqlx::query(
        "DELETE FROM access_control_entities WHERE entity_type = 'plugin' AND entity_id = $1",
    )
    .bind(&id)
    .execute(&*pool)
    .await
    .expect("delete entity");
    assert!(tasks.shutdown(Duration::from_secs(5)).await.is_drained());
    tokio::time::timeout(BOUND, bridge.shutdown())
        .await
        .expect("relay shutdown");
}
