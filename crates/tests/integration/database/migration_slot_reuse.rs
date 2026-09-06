//! Live-database tests for migration slot identity: a number that has been
//! spent may never be silently refilled.
//!
//! A migration file that is deleted leaves its tracking row behind in every
//! established database. The checksum alone cannot tell that apart from an
//! edit, because it hashes only the SQL — so the recorded `name` is what
//! distinguishes the two, and a `.tombstone` is what keeps the number claimed.

use std::env;
use std::sync::Arc;

use sqlx::{PgPool, Row, query};
use systemprompt_database::{
    Database, MigrationConfig, MigrationService, install_extension_schemas,
};
use systemprompt_extension::{
    Extension, ExtensionMetadata, ExtensionRegistry, LoaderError, Migration, SchemaDefinition,
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

struct SlotExt {
    id: &'static str,
    table: &'static str,
    schema_sql: &'static str,
    migrations: Vec<Migration>,
}

impl Extension for SlotExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "slot-reuse-test",
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
        self.migrations.clone()
    }
}

async fn applied_versions(pool: &PgPool, ext_id: &str) -> Vec<i32> {
    query("SELECT version FROM extension_migrations WHERE extension_id = $1 ORDER BY version")
        .bind(ext_id)
        .fetch_all(pool)
        .await
        .expect("read extension_migrations")
        .into_iter()
        .map(|r| r.get::<i32, _>("version"))
        .collect()
}

async fn column_exists(pool: &PgPool, table: &str, column: &str) -> bool {
    query(
        "SELECT 1 AS one FROM information_schema.columns WHERE table_schema = 'public' AND \
         table_name = $1 AND column_name = $2",
    )
    .bind(table)
    .bind(column)
    .fetch_optional(pool)
    .await
    .expect("column lookup must succeed")
    .is_some()
}

struct Fixture {
    db: Arc<Database>,
    pool: PgPool,
    ext_id: &'static str,
    table: &'static str,
    schema_sql: &'static str,
    create_sql: &'static str,
    reuse_sql: &'static str,
    _cleanup: Cleanup,
}

async fn fixture() -> Fixture {
    let db = Database::new_postgres(&database_url())
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak_str(format!("slot_test_{suffix}"));
    let ext_id: &'static str = leak_str(format!("slot-ext-{suffix}"));

    let cleanup = Cleanup {
        pool: pool.clone(),
        tables: vec![table],
        extension_ids: vec![ext_id],
    };

    Fixture {
        db: Arc::new(db),
        pool,
        ext_id,
        table,
        schema_sql: leak_str(format!(
            "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);"
        )),
        create_sql: leak_str(format!(
            "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);"
        )),
        reuse_sql: leak_str(format!(
            "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS refilled_slot INT;"
        )),
        _cleanup: cleanup,
    }
}

async fn apply_original(f: &Fixture) {
    let ext = SlotExt {
        id: f.ext_id,
        table: f.table,
        schema_sql: f.schema_sql,
        migrations: vec![Migration::new(7, "original", f.create_sql)],
    };
    MigrationService::new(f.db.write())
        .run_pending_migrations(&ext)
        .await
        .expect("first run applies version 7");
    assert_eq!(applied_versions(&f.pool, f.ext_id).await, vec![7]);
}

fn refilled(f: &Fixture) -> SlotExt {
    SlotExt {
        id: f.ext_id,
        table: f.table,
        schema_sql: f.schema_sql,
        migrations: vec![Migration::new(7, "refilled", f.reuse_sql)],
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reusing_a_spent_slot_is_refused_and_its_sql_never_runs() {
    let f = fixture().await;
    apply_original(&f).await;

    let err = MigrationService::new(f.db.write())
        .run_pending_migrations(&refilled(&f))
        .await
        .expect_err("a reused slot must be refused");

    match err {
        LoaderError::MigrationSlotReused {
            version,
            ref stored_name,
            ref current_name,
            ..
        } => {
            assert_eq!(version, 7);
            assert_eq!(stored_name, "original");
            assert_eq!(current_name, "refilled");
        },
        other => panic!("expected MigrationSlotReused, got {other:?}"),
    }

    assert!(
        !column_exists(&f.pool, f.table, "refilled_slot").await,
        "the refilling migration must not have executed"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn checksum_drift_flag_tolerates_a_reused_slot_without_running_it() {
    let f = fixture().await;
    apply_original(&f).await;

    MigrationService::new(f.db.write())
        .with_config(MigrationConfig {
            allow_checksum_drift: true,
        })
        .run_pending_migrations(&refilled(&f))
        .await
        .expect("--allow-checksum-drift tolerates the reuse");

    assert!(
        !column_exists(&f.pool, f.table, "refilled_slot").await,
        "tolerating the reuse still treats version 7 as applied, so its SQL is skipped"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tombstoned_slot_is_inert_and_later_migrations_still_run() {
    let f = fixture().await;
    apply_original(&f).await;

    let ext = SlotExt {
        id: f.ext_id,
        table: f.table,
        schema_sql: f.schema_sql,
        migrations: vec![
            Migration::tombstone(7, "original"),
            Migration::new(8, "later", f.reuse_sql),
        ],
    };

    let result = MigrationService::new(f.db.write())
        .run_pending_migrations(&ext)
        .await
        .expect("a tombstoned slot must not block the run");

    assert_eq!(result.migrations_run, 1, "only version 8 has SQL to run");
    assert_eq!(
        result.migrations_skipped, 0,
        "a tombstone is neither run nor counted as a skipped applied migration"
    );
    assert_eq!(applied_versions(&f.pool, f.ext_id).await, vec![7, 8]);
    assert!(column_exists(&f.pool, f.table, "refilled_slot").await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_fresh_database_stamps_real_migrations_but_not_tombstones() {
    let f = fixture().await;

    let ext = SlotExt {
        id: f.ext_id,
        table: f.table,
        schema_sql: f.schema_sql,
        migrations: vec![
            Migration::tombstone(1, "long_gone"),
            Migration::new(2, "kept", f.create_sql),
        ],
    };

    let mut registry = ExtensionRegistry::new();
    registry.register(Arc::new(ext)).expect("register");

    install_extension_schemas(&registry, f.db.as_ref())
        .await
        .expect("fresh install");

    assert_eq!(
        applied_versions(&f.pool, f.ext_id).await,
        vec![2],
        "a tombstone has no SQL, so a fresh database records nothing for its slot"
    );
}
