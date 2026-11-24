//! The live-schema digest an install reads to decide whether it is settled
//! completes while installs in other sessions create and drop relations,
//! indexes, views and functions: every install holds the bootstrap lock
//! exclusively and the digest holds it shared, so no install drops an object
//! the scan has listed. A scan that meets an install already holding the
//! lock waits for it, then succeeds.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use systemprompt_database::{BootstrapLockGuard, schema_catalog_digest};
use systemprompt_test_fixtures::DisposableDb;

const CHURN: [&str; 6] = [
    "CREATE TABLE churn.t (id INT PRIMARY KEY, note TEXT)",
    "CREATE INDEX churn_t_note ON churn.t (note)",
    "CREATE VIEW churn.v AS SELECT id, note FROM churn.t",
    "CREATE FUNCTION churn.f() RETURNS INT LANGUAGE sql AS 'SELECT 1'",
    "DROP FUNCTION churn.f()",
    "DROP TABLE churn.t CASCADE",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_digest_completes_while_another_session_drops_relations() {
    let database = DisposableDb::with_schema("digest_churn").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool().as_ref().clone();
    sqlx::query("CREATE SCHEMA churn")
        .execute(&raw)
        .await
        .expect("churn schema");

    let stop = Arc::new(AtomicBool::new(false));
    let churners: Vec<_> = (0..2)
        .map(|_| {
            let raw = raw.clone();
            let installer = pool.clone();
            let stop = Arc::clone(&stop);
            tokio::spawn(async move {
                let mut rounds = 0_u32;
                while !stop.load(Ordering::Relaxed) {
                    let install = BootstrapLockGuard::acquire(installer.write())
                        .await
                        .expect("install lock");
                    for sql in CHURN {
                        let _ = sqlx::query(sql).execute(&raw).await;
                    }
                    install.release().await;
                    rounds += 1;
                }
                rounds
            })
        })
        .collect();

    let mut failures = Vec::new();
    for _ in 0..150 {
        if let Err(error) = schema_catalog_digest(pool.write()).await {
            failures.push(format!("{error:?}"));
        }
    }
    stop.store(true, Ordering::Relaxed);
    let mut rounds = 0;
    for churner in churners {
        rounds += churner.await.expect("churner");
    }
    tokio::time::sleep(Duration::from_millis(10)).await;

    assert!(rounds > 0, "the churn ran beside the digests");
    assert!(
        failures.is_empty(),
        "{} of 150 digests failed under concurrent DDL; first: {}",
        failures.len(),
        failures.first().map_or("", String::as_str)
    );
    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_scan_waits_for_an_install_dropping_objects_then_succeeds() {
    let database = DisposableDb::with_schema("digest_drop_wait").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool().as_ref().clone();
    for sql in [
        "CREATE SCHEMA churn",
        "CREATE TABLE churn.t (id INT PRIMARY KEY, note TEXT)",
        "CREATE INDEX churn_t_note ON churn.t (note)",
        "CREATE VIEW churn.v AS SELECT id, note FROM churn.t",
        "CREATE FUNCTION churn.f() RETURNS INT LANGUAGE sql AS 'SELECT 1'",
    ] {
        sqlx::query(sql).execute(&raw).await.expect(sql);
    }

    let install = BootstrapLockGuard::acquire(pool.write())
        .await
        .expect("install lock");
    let scanner = pool.clone();
    let scan = tokio::spawn(async move { schema_catalog_digest(scanner.write()).await });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !scan.is_finished(),
        "the scan waits for the install holding the lock"
    );

    for sql in ["DROP FUNCTION churn.f()", "DROP TABLE churn.t CASCADE"] {
        sqlx::query(sql).execute(&raw).await.expect(sql);
    }
    install.release().await;

    scan.await
        .expect("scan task")
        .expect("the scan succeeds after the install's DROP … CASCADE");
    drop(pool);
    database.drop_now().await;
}
