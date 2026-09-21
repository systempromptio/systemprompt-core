use std::time::Duration;

use systemprompt_runtime::reporting::SnapshotWakeup;
use systemprompt_test_fixtures::DisposableDb;

#[tokio::test]
async fn notification_wakes_subscriber_and_shutdown_is_idempotent() {
    let database = DisposableDb::installed("snapshot_wakeup").await.unwrap();
    let db = database.pool().await.unwrap();
    let wakeup = SnapshotWakeup::default();
    let mut receiver = wakeup.subscribe(&db).await;

    let pool = db.write_pool_arc().unwrap();
    let observed = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            sqlx::query("SELECT pg_notify('feedback_snapshots', 'test')")
                .execute(pool.as_ref())
                .await
                .unwrap();
            if matches!(
                tokio::time::timeout(Duration::from_millis(75), receiver.changed()).await,
                Ok(Ok(()))
            ) {
                break;
            }
        }
    })
    .await;
    assert!(observed.is_ok(), "snapshot notification was not delivered");
    tokio::time::timeout(Duration::from_secs(1), wakeup.shutdown())
        .await
        .expect("first shutdown should join the relay");
    receiver.borrow_and_update();
    sqlx::query("SELECT pg_notify('feedback_snapshots', 'after-shutdown')")
        .execute(pool.as_ref())
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(250), receiver.changed())
            .await
            .is_err(),
        "a stopped relay must not deliver notifications"
    );

    let mut restarted = wakeup.subscribe(&db).await;
    let old_generation = *receiver.borrow();
    let new_generation = *restarted.borrow();
    let observed_after_restart = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            sqlx::query("SELECT pg_notify('feedback_snapshots', 'after-restart')")
                .execute(pool.as_ref())
                .await
                .unwrap();
            let old = tokio::time::timeout(Duration::from_millis(75), receiver.changed()).await;
            let new = tokio::time::timeout(Duration::from_millis(75), restarted.changed()).await;
            if matches!(old, Ok(Ok(()))) && matches!(new, Ok(Ok(()))) {
                break;
            }
        }
    })
    .await;
    assert!(
        observed_after_restart.is_ok(),
        "a restarted relay must notify every subscriber"
    );
    assert!(*receiver.borrow() > old_generation);
    assert!(*restarted.borrow() > new_generation);

    tokio::time::timeout(Duration::from_secs(1), wakeup.shutdown())
        .await
        .expect("second shutdown should be harmless");
    tokio::time::timeout(Duration::from_secs(1), wakeup.shutdown())
        .await
        .expect("shutdown remains harmless after a restart");

    drop(pool);
    drop(db);
    database.drop_now().await;
}

#[tokio::test]
async fn exhausted_listener_pool_recovers_and_delivers_a_later_notification() {
    use sqlx::postgres::PgPoolOptions;
    use std::sync::Arc;

    let database = DisposableDb::create("snapshot_wakeup_pool_recovery")
        .await
        .expect("private snapshot database");
    let constrained = PgPoolOptions::new()
        .max_connections(1)
        .connect(database.url())
        .await
        .expect("single-connection listener pool");
    let db = Arc::new(systemprompt_database::Database::from_pools(
        Arc::new(constrained.clone()),
        None,
    ));
    let held = constrained
        .acquire()
        .await
        .expect("hold listener connection");
    let wakeup = SnapshotWakeup::default();
    let mut receiver = wakeup.subscribe(&db).await;
    let initial = *receiver.borrow_and_update();

    assert!(
        tokio::time::timeout(Duration::from_millis(2_400), receiver.changed())
            .await
            .is_err(),
        "a listener connection timeout must not manufacture a wakeup generation"
    );
    assert_eq!(*receiver.borrow_and_update(), initial);

    drop(held);
    let notifier = PgPoolOptions::new()
        .max_connections(1)
        .connect(database.url())
        .await
        .expect("independent notification pool");
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            sqlx::query("SELECT pg_notify('feedback_snapshots', 'recovered')")
                .execute(&notifier)
                .await
                .expect("publish recovery notification");
            if matches!(
                tokio::time::timeout(Duration::from_millis(150), receiver.changed()).await,
                Ok(Ok(()))
            ) {
                break;
            }
        }
    })
    .await
    .expect("listener reconnects and receives a later notification");
    assert!(*receiver.borrow_and_update() > initial);

    tokio::time::timeout(Duration::from_secs(1), wakeup.shutdown())
        .await
        .expect("shutdown recovered relay");
    notifier.close().await;
    constrained.close().await;
    drop(db);
    database.drop_now().await;
}
