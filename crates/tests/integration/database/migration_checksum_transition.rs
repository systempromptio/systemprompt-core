//! Historical checksum conversion must never become migration drift repair.
use super::*;
use std::hash::{Hash, Hasher};

fn historical(sql: &str) -> String {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    sql.hash(&mut hash);
    format!("{:x}", hash.finish())
}
async fn stored(f: &Fixture, version: i32) -> String {
    query("SELECT checksum FROM extension_migrations WHERE extension_id=$1 AND version=$2")
        .bind(f.ext_id)
        .bind(version)
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("checksum")
}
async fn historical_row(f: &Fixture, version: i32, sql: &str) {
    query("UPDATE extension_migrations SET checksum=$1 WHERE extension_id=$2 AND version=$3")
        .bind(historical(sql))
        .bind(f.ext_id)
        .bind(version)
        .execute(&f.pool)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exact_historical_hash_converts_without_reexecution_or_timestamp_change() {
    let f = fixture().await;
    let sql = leak_str(format!(
        "{} INSERT INTO {}(id) VALUES ('must-not-run-twice');",
        f.create_sql, f.table
    ));
    let migration = Migration::new(7, "original", sql);
    let ext = SlotExt {
        id: f.ext_id,
        table: f.table,
        schema_sql: f.schema_sql,
        migrations: vec![migration.clone()],
    };
    let service = MigrationService::new(f.db.write());
    service.run_pending_migrations(&ext).await.unwrap();
    historical_row(&f, 7, sql).await;
    let before: String =
        query("SELECT applied_at::text AS value FROM extension_migrations WHERE extension_id=$1")
            .bind(f.ext_id)
            .fetch_one(&f.pool)
            .await
            .unwrap()
            .get("value");
    let status = service.status(&ext).await.unwrap();
    assert!(status.drift.is_empty());
    assert_eq!(
        stored(&f, 7).await,
        historical(sql),
        "status must remain read-only"
    );
    let result = service
        .run_pending_migrations(&ext)
        .await
        .expect("exact historical checksum is eligible");
    assert_eq!((result.migrations_run, result.migrations_skipped), (0, 1));
    assert_eq!(stored(&f, 7).await, migration.checksum());
    let after: String =
        query("SELECT applied_at::text AS value FROM extension_migrations WHERE extension_id=$1")
            .bind(f.ext_id)
            .fetch_one(&f.pool)
            .await
            .unwrap()
            .get("value");
    assert_eq!(before, after);
    let count: i64 = query(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) AS n FROM {}",
        f.table
    )))
    .fetch_one(&f.pool)
    .await
    .unwrap()
    .get("n");
    assert_eq!(
        count, 1,
        "non-idempotent original SQL must not execute again"
    );
    let repeated = service.run_pending_migrations(&ext).await.unwrap();
    assert_eq!(
        (repeated.migrations_run, repeated.migrations_skipped),
        (0, 1)
    );
    assert_eq!(stored(&f, 7).await, migration.checksum());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edited_sql_rejects_entire_extension_before_any_checksum_transition() {
    let f = fixture().await;
    let original = Migration::new(7, "original", f.create_sql);
    let second = Migration::new(8, "second", "SELECT 8;");
    let mut ext = SlotExt {
        id: f.ext_id,
        table: f.table,
        schema_sql: f.schema_sql,
        migrations: vec![original.clone(), second.clone()],
    };
    let service = MigrationService::new(f.db.write());
    service.run_pending_migrations(&ext).await.unwrap();
    historical_row(&f, 7, original.sql).await;
    historical_row(&f, 8, second.sql).await;
    ext.migrations[1] = Migration::new(8, "second", "SELECT 9;");
    assert_eq!(service.status(&ext).await.unwrap().drift.len(), 1);
    let error = service
        .run_pending_migrations(&ext)
        .await
        .expect_err("edited SQL is real drift");
    assert!(error.to_string().contains("edited"));
    assert_eq!(stored(&f, 7).await, historical(original.sql));
    assert_eq!(stored(&f, 8).await, historical(second.sql));
    ext.migrations[1] = second.clone();
    service
        .run_pending_migrations(&ext)
        .await
        .expect("restoring exact original SQL permits bookkeeping transition");
    assert_eq!(stored(&f, 7).await, original.checksum());
    assert_eq!(stored(&f, 8).await, second.checksum());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn historical_hash_does_not_authorize_reused_or_tombstoned_slots() {
    let f = fixture().await;
    apply_original(&f).await;
    historical_row(&f, 7, f.create_sql).await;
    let mut ext = SlotExt {
        id: f.ext_id,
        table: f.table,
        schema_sql: f.schema_sql,
        migrations: vec![Migration::new(7, "renamed-slot", f.create_sql)],
    };
    let service = MigrationService::new(f.db.write());
    assert!(matches!(
        service.run_pending_migrations(&ext).await,
        Err(LoaderError::MigrationSlotReused { .. })
    ));
    assert_eq!(stored(&f, 7).await, historical(f.create_sql));
    ext.migrations = vec![Migration::tombstone(7, "retired")];
    let result = service.run_pending_migrations(&ext).await.unwrap();
    assert_eq!((result.migrations_run, result.migrations_skipped), (0, 0));
    assert_eq!(stored(&f, 7).await, historical(f.create_sql));
    assert!(service.status(&ext).await.unwrap().tombstoned[0].tracked);
    ext.migrations.clear();
    assert_eq!(service.status(&ext).await.unwrap().orphaned.len(), 1);
    assert_eq!(stored(&f, 7).await, historical(f.create_sql));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_history_replacement_rolls_back_earlier_conversions() {
    let f = fixture().await;
    let first = Migration::new(7, "original", f.create_sql);
    let second = Migration::new(8, "second", "SELECT 8;");
    let ext = SlotExt {
        id: f.ext_id,
        table: f.table,
        schema_sql: f.schema_sql,
        migrations: vec![first.clone(), second.clone()],
    };
    MigrationService::new(f.db.write())
        .run_pending_migrations(&ext)
        .await
        .unwrap();
    historical_row(&f, 7, first.sql).await;
    historical_row(&f, 8, second.sql).await;
    let mut writer = f.pool.begin().await.unwrap();
    query(
        "SELECT version FROM extension_migrations WHERE extension_id=$1 AND version=8 FOR UPDATE",
    )
    .bind(f.ext_id)
    .fetch_one(&mut *writer)
    .await
    .unwrap();
    let db = f.db.clone();
    let mut runner = tokio::spawn(async move {
        MigrationService::new(db.write())
            .run_pending_migrations(&ext)
            .await
    });
    // The runner's first transition is visible as a held row lock, while its
    // second transition waits for our uncommitted history replacement.
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let mut observer = f.pool.begin().await.unwrap();
            let attempt = query("SELECT version FROM extension_migrations WHERE extension_id=$1 AND version=7 FOR UPDATE NOWAIT")
                .bind(f.ext_id).fetch_one(&mut *observer).await;
            observer.rollback().await.unwrap();
            if let Err(sqlx::Error::Database(error)) = attempt {
                assert_eq!(error.code().as_deref(), Some("55P03"));
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("runner must reach the fenced transition");
    query("UPDATE extension_migrations SET checksum='concurrent-edit' WHERE extension_id=$1 AND version=8")
        .bind(f.ext_id).execute(&mut *writer).await.unwrap();
    writer.commit().await.unwrap();
    let error = tokio::time::timeout(std::time::Duration::from_secs(10), &mut runner)
        .await
        .expect("bounded runner")
        .unwrap()
        .expect_err("CAS must reject replaced history");
    assert!(error.to_string().contains("concurrently"));
    assert_eq!(
        stored(&f, 7).await,
        historical(first.sql),
        "first rewrite rolled back with second mismatch"
    );
    assert_eq!(stored(&f, 8).await, "concurrent-edit");
}
