use systemprompt_database::{BOOTSTRAP_ADVISORY_LOCK_KEY, BootstrapLockGuard, PostgresProvider};

#[tokio::test]
async fn bootstrap_lock_acquisition_reports_closed_pool_without_holding_a_session() {
    let db = systemprompt_test_fixtures::closed_db_pool().await;
    let provider = PostgresProvider::from_pool(db.write_pool());

    let error = BootstrapLockGuard::acquire(&provider)
        .await
        .expect_err("closed pool cannot acquire the bootstrap lock session");
    let message = error.to_string();
    assert!(message.contains("database"), "{message}");
    assert!(message.contains("bootstrap lock connection"), "{message}");
}

#[tokio::test]
async fn bootstrap_lock_release_discards_a_terminated_session_and_a_fresh_guard_recovers() {
    let database =
        systemprompt_test_fixtures::DisposableDb::installed("bootstrap_lock_terminated_session")
            .await
            .expect("private bootstrap-lock database");
    let db = database.pool().await.expect("private database pool");
    let provider = PostgresProvider::from_pool(db.write_pool());
    let guard = BootstrapLockGuard::acquire(&provider)
        .await
        .expect("acquire bootstrap lock on owned session");
    let pool = db.pool_arc().expect("private read pool");
    let lock_pid: i32 = sqlx::query_scalar(
        "SELECT l.pid FROM pg_locks l \
         JOIN pg_database d ON d.oid=l.database \
         WHERE l.locktype='advisory' AND l.granted AND d.datname=current_database() \
         AND l.classid=(($1::bigint >> 32) & 4294967295)::oid \
         AND l.objid=($1::bigint & 4294967295)::oid AND l.objsubid=1 \
         AND l.pid <> pg_backend_pid()",
    )
    .bind(BOOTSTRAP_ADVISORY_LOCK_KEY)
    .fetch_one(pool.as_ref())
    .await
    .expect("exact owned advisory-lock backend");
    let owned_database: String =
        sqlx::query_scalar("SELECT datname FROM pg_stat_activity WHERE pid=$1")
            .bind(lock_pid)
            .fetch_one(pool.as_ref())
            .await
            .expect("lock backend identity");
    let current_database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(pool.as_ref())
        .await
        .expect("fixture database identity");
    assert_eq!(owned_database, current_database);

    let terminated: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1)")
        .bind(lock_pid)
        .fetch_one(pool.as_ref())
        .await
        .expect("terminate only the owned lock backend");
    assert!(
        terminated,
        "PostgreSQL accepted termination of the owned session"
    );
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let present: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1 AND datname=current_database())",
            )
            .bind(lock_pid)
            .fetch_one(pool.as_ref())
            .await
            .expect("observe terminated owned backend");
            if !present {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("owned backend exits within timeout");

    guard.release().await;

    let recovered = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        BootstrapLockGuard::acquire(&provider),
    )
    .await
    .expect("fresh lock acquisition does not block on a stale pooled session")
    .expect("fresh bootstrap guard reacquires after terminated holder");
    let value: i32 = sqlx::query_scalar("SELECT 1")
        .fetch_one(pool.as_ref())
        .await
        .expect("pool remains usable after failed release");
    assert_eq!(value, 1);
    recovered.release().await;

    drop(pool);
    drop(provider);
    drop(db);
    database.drop_now().await;
}
