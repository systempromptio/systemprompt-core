use std::time::{Duration, Instant};

use systemprompt_database::{BootstrapLockGuard, BootstrapLockTimeout, PostgresProvider};
use systemprompt_test_fixtures::DisposableDb;

#[tokio::test]
async fn a_held_bootstrap_lock_times_out_with_a_typed_error_after_the_bound() {
    let database = DisposableDb::empty("bootstrap_lock_wait").await;
    let db = database.test_pool().await;
    let provider = PostgresProvider::from_pool(db.write_pool());

    let holder = BootstrapLockGuard::acquire(&provider)
        .await
        .expect("first session takes the lock");

    let started = Instant::now();
    let waited = tokio::time::timeout(
        Duration::from_secs(20),
        BootstrapLockGuard::acquire_within(&provider, Duration::from_secs(1)),
    )
    .await
    .expect("a bounded wait must return, not hang");
    let error = waited.expect_err("the lock is held by another session");
    assert!(started.elapsed() >= Duration::from_secs(1));

    let mut source: Option<&dyn std::error::Error> = Some(&error);
    let mut timeout = None;
    while let Some(current) = source {
        if let Some(t) = current.downcast_ref::<BootstrapLockTimeout>() {
            timeout = Some(*t);
        }
        source = current.source();
    }
    assert_eq!(
        timeout.expect("the cause is a BootstrapLockTimeout").waited,
        Duration::from_secs(1)
    );

    holder.release().await;
    let after = BootstrapLockGuard::acquire_within(&provider, Duration::from_secs(5))
        .await
        .expect("a released lock is acquired");
    after.release().await;
    drop(db);
    database.drop_now().await;
}
