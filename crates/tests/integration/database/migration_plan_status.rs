//! Live-database tests for `MigrationService::plan_pending` and
//! `MigrationService::status`. Exercises the dry-run plan path (no DB
//! writes) and the introspectable status path (applied / pending / drift).

use std::env;
use std::sync::Arc;

use sqlx::{PgPool, query};
use systemprompt_database::{Database, MigrationService, install_extension_schemas};
use systemprompt_extension::{
    Extension, ExtensionMetadata, ExtensionRegistry, Migration, SchemaDefinition,
};
use uuid::Uuid;

const DEFAULT_DATABASE_URL: &str = "postgres://systemprompt_admin:\
                                    3e00fcdac26b5b731829e8737515db8f@localhost:5432/\
                                    systemprompt-web";

fn database_url() -> String {
    env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string())
}

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn fresh_suffix() -> String {
    Uuid::new_v4().simple().to_string()[..12].to_string()
}

struct Cleanup {
    pool: PgPool,
    tables: Vec<&'static str>,
    extension_ids: Vec<&'static str>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        let tables = self.tables.clone();
        let extension_ids = self.extension_ids.clone();
        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                for t in &tables {
                    let _ = query(sqlx::AssertSqlSafe(format!(
                        "DROP TABLE IF EXISTS {t} CASCADE"
                    )))
                    .execute(&pool)
                    .await;
                }
                for ext_id in &extension_ids {
                    let _ = query("DELETE FROM extension_migrations WHERE extension_id = $1")
                        .bind(*ext_id)
                        .execute(&pool)
                        .await;
                }
            });
        });
    }
}

struct TwoMigrationExt {
    id: &'static str,
    schema_sql: &'static str,
    table: &'static str,
    m1: &'static str,
    m2: &'static str,
}

impl Extension for TwoMigrationExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "two-mig-test",
            version: "0.0.0",
        }
    }

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_pending_lists_all_then_none_after_apply() {
    let url = database_url();
    let db = Database::new_postgres(&url)
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak_str(format!("plan_test_{suffix}"));
    let ext_id: &'static str = leak_str(format!("plan-ext-{suffix}"));

    let _cleanup = Cleanup {
        pool: pool.clone(),
        tables: vec![table],
        extension_ids: vec![ext_id],
    };

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

    let ext = TwoMigrationExt {
        id: ext_id,
        schema_sql,
        table,
        m1,
        m2,
    };

    let db_arc = Arc::new(db);
    let svc = MigrationService::new(db_arc.write());

    let plan_before = svc
        .plan_pending(&ext)
        .await
        .expect("plan_pending must succeed");
    assert_eq!(plan_before.len(), 2, "both migrations should be pending");
    assert_eq!(plan_before[0].version, 1);
    assert_eq!(plan_before[1].version, 2);
    assert_eq!(plan_before[0].extension_id, ext_id);
    assert!(!plan_before[0].checksum.is_empty());
    assert!(!plan_before[0].sql.is_empty());

    let mut registry = ExtensionRegistry::new();
    registry
        .register(Arc::new(TwoMigrationExt {
            id: ext_id,
            schema_sql,
            table,
            m1,
            m2,
        }))
        .expect("register");

    install_extension_schemas(&registry, db_arc.as_ref())
        .await
        .expect("install");

    let plan_after = svc
        .plan_pending(&ext)
        .await
        .expect("plan_pending after install must succeed");
    assert!(
        plan_after.is_empty(),
        "after install, no migrations should be pending; got {plan_after:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_reports_applied_pending_and_drift() {
    let url = database_url();
    let db = Database::new_postgres(&url)
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak_str(format!("status_test_{suffix}"));
    let ext_id: &'static str = leak_str(format!("status-ext-{suffix}"));

    let _cleanup = Cleanup {
        pool: pool.clone(),
        tables: vec![table],
        extension_ids: vec![ext_id],
    };

    let schema_sql: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);"
    ));
    let m1: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);"
    ));
    let m2: &'static str = leak_str(format!(
        "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS payload JSONB;"
    ));

    let ext_v1_only = TwoMigrationExt {
        id: ext_id,
        schema_sql,
        table,
        m1,
        m2,
    };

    let mut registry = ExtensionRegistry::new();
    let single_ext = SingleMigrationExt {
        id: ext_id,
        schema_sql,
        table,
        m1,
    };
    registry
        .register(Arc::new(single_ext))
        .expect("register single-mig");

    let db_arc = Arc::new(db);
    install_extension_schemas(&registry, db_arc.as_ref())
        .await
        .expect("install m1 only");

    let svc = MigrationService::new(db_arc.write());
    let status = svc.status(&ext_v1_only).await.expect("status must succeed");

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

    let drifted = svc.status(&ext_v1_only).await.expect("status after tamper");

    assert_eq!(drifted.drift.len(), 1, "v1 should be flagged as drifted");
    assert_eq!(drifted.drift[0].version, 1);
    assert_eq!(drifted.drift[0].stored_checksum, "tampered");
    assert_ne!(
        drifted.drift[0].stored_checksum,
        drifted.drift[0].current_checksum
    );
    assert_eq!(drifted.applied.len(), 1);
    assert_eq!(drifted.pending.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repair_drift_reconciles_tampered_checksum() {
    let url = database_url();
    let db = Database::new_postgres(&url)
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak_str(format!("repair_test_{suffix}"));
    let ext_id: &'static str = leak_str(format!("repair-ext-{suffix}"));

    let _cleanup = Cleanup {
        pool: pool.clone(),
        tables: vec![table],
        extension_ids: vec![ext_id],
    };

    let schema_sql: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);"
    ));
    let m1: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);"
    ));

    let ext = SingleMigrationExt {
        id: ext_id,
        schema_sql,
        table,
        m1,
    };

    let mut registry = ExtensionRegistry::new();
    registry
        .register(Arc::new(SingleMigrationExt {
            id: ext_id,
            schema_sql,
            table,
            m1,
        }))
        .expect("register");

    let db_arc = Arc::new(db);
    install_extension_schemas(&registry, db_arc.as_ref())
        .await
        .expect("install");

    let svc = MigrationService::new(db_arc.write());

    let noop = svc.repair_drift(&ext).await.expect("repair with no drift");
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
        svc.status(&ext).await.expect("status").drift.len(),
        1,
        "tampering should produce drift"
    );

    let repaired = svc.repair_drift(&ext).await.expect("repair_drift");
    assert_eq!(repaired.repaired.len(), 1);
    assert_eq!(repaired.repaired[0].version, 1);
    assert_eq!(repaired.repaired[0].stored_checksum, "tampered");
    assert_eq!(
        repaired.migrations_run, 1,
        "the drifted migration is re-applied"
    );

    let after = svc.status(&ext).await.expect("status after repair");
    assert!(after.drift.is_empty(), "drift reconciled by repair");
    assert_eq!(after.applied.len(), 1);
}

struct SingleMigrationExt {
    id: &'static str,
    schema_sql: &'static str,
    table: &'static str,
    m1: &'static str,
}

impl Extension for SingleMigrationExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "single-mig-test",
            version: "0.0.0",
        }
    }

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
