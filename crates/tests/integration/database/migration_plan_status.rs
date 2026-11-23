//! Live-database tests for `MigrationService::plan_pending` and
//! `MigrationService::status`. Exercises the dry-run plan path (no DB
//! writes) and the introspectable status path (applied / pending / drift).

use std::sync::Arc;

use sqlx::{PgPool, query};
use systemprompt_database::{Database, MigrationService, SchemaMember};
use systemprompt_extension::{
    ContributionError, Contributions, Extension, ExtensionMetadata, Migration, SchemaDefinition,
};
use systemprompt_test_fixtures::DisposableDb;

use crate::composition::{install, member};
use uuid::Uuid;

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn fresh_suffix() -> String {
    Uuid::new_v4().simple().to_string()[..12].to_string()
}

struct TwoMigrationExt {
    id: &'static str,
    schema_sql: &'static str,
    table: &'static str,
    m1: &'static str,
    m2: &'static str,
}

impl TwoMigrationExt {
    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::new(
            self.table.to_string(),
            self.schema_sql.to_string(),
        )]
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![
            Migration::new(1, "first", self.m1),
            Migration::new(2, "second", self.m2),
        ]
    }
}

impl Extension for TwoMigrationExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "two-mig-test",
            version: "0.0.0",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        for schema in self.schemas() {
            c.schema(schema);
        }
        c.migrations(self.migrations());
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_pending_lists_all_then_none_after_apply() {
    let database = DisposableDb::with_schema("migration_plan_status").await;
    let db = Database::new_postgres(database.url())
        .await
        .expect("connect to the disposable database");
    let pool: PgPool = db.pool().as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak_str(format!("plan_test_{suffix}"));
    let ext_id: &'static str = leak_str(format!("plan-ext-{suffix}"));

    let schema_sql: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY, a INT, b INT);"
    ));
    let m1: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);"
    ));
    let m2: &'static str = leak_str(format!(
        "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS a INT; ALTER TABLE {table} ADD COLUMN IF \
         NOT EXISTS b INT;"
    ));

    let ext = member(TwoMigrationExt {
        id: ext_id,
        schema_sql,
        table,
        m1,
        m2,
    });

    let db_arc = Arc::new(db);
    let svc = MigrationService::new(db_arc.write());

    let plan_before = svc
        .plan_pending(ext.as_ref())
        .await
        .expect("plan_pending must succeed");
    assert_eq!(plan_before.len(), 2, "both migrations should be pending");
    assert_eq!(plan_before[0].version, 1);
    assert_eq!(plan_before[1].version, 2);
    assert_eq!(plan_before[0].extension_id, ext_id);
    assert!(!plan_before[0].checksum.is_empty());
    assert!(!plan_before[0].sql.is_empty());

    let members: Vec<Box<dyn Extension>> = vec![Box::new(TwoMigrationExt {
        id: ext_id,
        schema_sql,
        table,
        m1,
        m2,
    })];

    install(members, db_arc.as_ref()).await.expect("install");

    let plan_after = svc
        .plan_pending(ext.as_ref())
        .await
        .expect("plan_pending after install must succeed");
    assert!(
        plan_after.is_empty(),
        "after install, no migrations should be pending; got {plan_after:?}"
    );
    pool.close().await;
    drop(db_arc);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_reports_applied_pending_and_drift() {
    let database = DisposableDb::with_schema("migration_plan_status").await;
    let db = Database::new_postgres(database.url())
        .await
        .expect("connect to the disposable database");
    let pool: PgPool = db.pool().as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak_str(format!("status_test_{suffix}"));
    let ext_id: &'static str = leak_str(format!("status-ext-{suffix}"));

    let schema_sql: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);"
    ));
    let m1: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);"
    ));
    let m2: &'static str = leak_str(format!(
        "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS payload JSONB;"
    ));

    let ext_v1_only = member(TwoMigrationExt {
        id: ext_id,
        schema_sql,
        table,
        m1,
        m2,
    });

    let members: Vec<Box<dyn Extension>> = vec![Box::new(SingleMigrationExt {
        id: ext_id,
        schema_sql,
        table,
        m1,
    })];

    let db_arc = Arc::new(db);
    install(members, db_arc.as_ref())
        .await
        .expect("install m1 only");

    let svc = MigrationService::new(db_arc.write());
    let status = svc
        .status(ext_v1_only.as_ref())
        .await
        .expect("status must succeed");

    assert_eq!(status.extension_id, ext_id);
    assert_eq!(status.applied.len(), 1);
    assert_eq!(status.applied[0].version, 1);
    assert!(
        status.applied[0].applied_at.is_some(),
        "applied_at should be populated from DB"
    );
    assert_eq!(status.pending.len(), 1);
    assert_eq!(status.pending[0].version, 2);
    assert!(status.drift.is_empty());

    query(
        "UPDATE extension_migrations SET checksum = 'tampered' WHERE extension_id = $1 AND \
         version = 1",
    )
    .bind(ext_id)
    .execute(&pool)
    .await
    .expect("tamper checksum");

    let drifted = svc
        .status(ext_v1_only.as_ref())
        .await
        .expect("status after tamper");

    assert_eq!(drifted.drift.len(), 1, "v1 should be flagged as drifted");
    assert_eq!(drifted.drift[0].version, 1);
    assert_eq!(drifted.drift[0].stored_checksum, "tampered");
    assert_ne!(
        drifted.drift[0].stored_checksum,
        drifted.drift[0].current_checksum
    );
    assert_eq!(drifted.applied.len(), 1);
    assert_eq!(drifted.pending.len(), 1);
    pool.close().await;
    drop(db_arc);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repair_drift_reconciles_tampered_checksum() {
    let database = DisposableDb::with_schema("migration_plan_status").await;
    let db = Database::new_postgres(database.url())
        .await
        .expect("connect to the disposable database");
    let pool: PgPool = db.pool().as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak_str(format!("repair_test_{suffix}"));
    let ext_id: &'static str = leak_str(format!("repair-ext-{suffix}"));

    let schema_sql: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);"
    ));
    let m1: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);"
    ));

    let ext = member(SingleMigrationExt {
        id: ext_id,
        schema_sql,
        table,
        m1,
    });

    let members: Vec<Box<dyn Extension>> = vec![Box::new(SingleMigrationExt {
        id: ext_id,
        schema_sql,
        table,
        m1,
    })];

    let db_arc = Arc::new(db);
    install(members, db_arc.as_ref()).await.expect("install");

    let svc = MigrationService::new(db_arc.write());

    let noop = svc
        .repair_drift(ext.as_ref())
        .await
        .expect("repair with no drift");
    assert!(noop.repaired.is_empty(), "no drift means nothing repaired");
    assert_eq!(noop.migrations_run, 0);

    query(
        "UPDATE extension_migrations SET checksum = 'tampered' WHERE extension_id = $1 AND \
         version = 1",
    )
    .bind(ext_id)
    .execute(&pool)
    .await
    .expect("tamper checksum");

    assert_eq!(
        svc.status(ext.as_ref()).await.expect("status").drift.len(),
        1,
        "tampering should produce drift"
    );

    let repaired = svc.repair_drift(ext.as_ref()).await.expect("repair_drift");
    assert_eq!(repaired.repaired.len(), 1);
    assert_eq!(repaired.repaired[0].version, 1);
    assert_eq!(repaired.repaired[0].stored_checksum, "tampered");
    assert_eq!(
        repaired.migrations_run, 0,
        "drifted migrations are re-applied in place, not counted as pending runs"
    );

    let after = svc.status(ext.as_ref()).await.expect("status after repair");
    assert!(after.drift.is_empty(), "drift reconciled by repair");
    assert_eq!(after.applied.len(), 1);
    pool.close().await;
    drop(db_arc);
    database.drop_now().await;
}

struct SingleMigrationExt {
    id: &'static str,
    schema_sql: &'static str,
    table: &'static str,
    m1: &'static str,
}

impl SingleMigrationExt {
    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::new(
            self.table.to_string(),
            self.schema_sql.to_string(),
        )]
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![Migration::new(1, "first", self.m1)]
    }
}

impl Extension for SingleMigrationExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "single-mig-test",
            version: "0.0.0",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        for schema in self.schemas() {
            c.schema(schema);
        }
        c.migrations(self.migrations());
        Ok(())
    }
}

async fn install_tampered_insert_ext(
    url: &str,
    pool: &PgPool,
    suffix: &str,
) -> (
    Arc<Database>,
    Arc<dyn SchemaMember>,
    &'static str,
    &'static str,
) {
    let table: &'static str = leak_str(format!("repair_ins_{suffix}"));
    let ext_id: &'static str = leak_str(format!("repair-ins-{suffix}"));
    let schema_sql: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);"
    ));
    let m1: &'static str = leak_str(format!("INSERT INTO {table} (id) VALUES ('seed');"));

    let ext = member(SingleMigrationExt {
        id: ext_id,
        schema_sql,
        table,
        m1,
    });

    let members: Vec<Box<dyn Extension>> = vec![Box::new(SingleMigrationExt {
        id: ext_id,
        schema_sql,
        table,
        m1,
    })];

    let db = Database::new_postgres(url)
        .await
        .expect("connect to the disposable database");
    let db_arc = Arc::new(db);
    install(members, db_arc.as_ref()).await.expect("install");

    query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO {table} (id) VALUES ('seed') ON CONFLICT DO NOTHING"
    )))
    .execute(pool)
    .await
    .expect("seed row");

    query(
        "UPDATE extension_migrations SET checksum = 'tampered' WHERE extension_id = $1 AND \
         version = 1",
    )
    .bind(ext_id)
    .execute(pool)
    .await
    .expect("tamper checksum");

    (db_arc, ext, table, ext_id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_reapply_keeps_tracking_row() {
    let database = DisposableDb::with_schema("migration_plan_status").await;
    let db = Database::new_postgres(database.url())
        .await
        .expect("connect to the disposable database");
    let pool: PgPool = db.pool().as_ref().clone();

    let suffix = fresh_suffix();
    let (db_arc, ext, _table, _ext_id) =
        install_tampered_insert_ext(database.url(), &pool, &suffix).await;

    let svc = MigrationService::new(db_arc.write());

    svc.repair_drift(ext.as_ref())
        .await
        .expect_err("re-applying a non-idempotent INSERT must fail on the duplicate key");

    let after = svc
        .status(ext.as_ref())
        .await
        .expect("status after failed repair");
    assert_eq!(
        after.applied.len(),
        1,
        "the tracking row must survive a failed re-apply"
    );
    assert_eq!(after.applied[0].checksum, "tampered");
    assert_eq!(after.drift.len(), 1, "drift is still reported, not lost");
    assert!(
        after.pending.is_empty(),
        "a failed re-apply must never turn an applied migration into a pending one"
    );
    drop(ext);
    pool.close().await;
    drop(db);
    drop(db_arc);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconcile_drift_rewrites_checksum_without_executing_sql() {
    let database = DisposableDb::with_schema("migration_plan_status").await;
    let db = Database::new_postgres(database.url())
        .await
        .expect("connect to the disposable database");
    let pool: PgPool = db.pool().as_ref().clone();

    let suffix = fresh_suffix();
    let (db_arc, ext, table, _ext_id) =
        install_tampered_insert_ext(database.url(), &pool, &suffix).await;

    let svc = MigrationService::new(db_arc.write());

    let result = svc
        .reconcile_drift(ext.as_ref())
        .await
        .expect("reconcile_drift");
    assert_eq!(result.repaired.len(), 1);
    assert_eq!(result.migrations_run, 0);

    let after = svc
        .status(ext.as_ref())
        .await
        .expect("status after reconcile");
    assert!(after.drift.is_empty(), "checksum rewritten in place");
    assert_eq!(after.applied.len(), 1);

    let count: i64 =
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
            .fetch_one(&pool)
            .await
            .expect("count rows");
    assert_eq!(
        count, 1,
        "reconcile must not execute the migration SQL (the INSERT would have failed or added a \
         row)"
    );
    drop(ext);
    pool.close().await;
    drop(db);
    drop(db_arc);
    database.drop_now().await;
}
