//! Invariant under test: on a fresh database — no `extension_migrations`
//! history for the extension and none of its owned tables present — the
//! installer stamps every defined migration as applied without executing its
//! SQL, and the declarative schema alone materialises the target state. An
//! established database (any owned table already present, with or without
//! tracking rows) executes migrations normally.

use std::env;
use std::sync::Arc;

use sqlx::{PgPool, Row};
use systemprompt_database::{Database, install_extension_schemas};
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

struct CanaryExtension {
    id: &'static str,
    schema_sql: &'static str,
    table: &'static str,
    migration_sql: &'static str,
}

impl Extension for CanaryExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "canary-test",
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
        vec![Migration::new(1, "canary", self.migration_sql)]
    }
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
                    let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
                        "DROP TABLE IF EXISTS {t} CASCADE"
                    )))
                    .execute(&pool)
                    .await;
                }
                for ext_id in &extension_ids {
                    let _ = sqlx::query("DELETE FROM extension_migrations WHERE extension_id = $1")
                        .bind(*ext_id)
                        .execute(&pool)
                        .await;
                }
            });
        });
    }
}

async fn applied_versions(pool: &PgPool, ext_id: &str) -> Vec<i32> {
    sqlx::query("SELECT version FROM extension_migrations WHERE extension_id = $1 ORDER BY version")
        .bind(ext_id)
        .fetch_all(pool)
        .await
        .expect("query applied")
        .into_iter()
        .map(|r| r.get::<i32, _>("version"))
        .collect()
}

async fn column_exists(pool: &PgPool, table: &str, column: &str) -> bool {
    sqlx::query(
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_database_stamps_migrations_without_executing_them() {
    let url = database_url();
    let db = Database::new_postgres(&url)
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak_str(format!("stamp_fresh_{suffix}"));
    let ext_id: &'static str = leak_str(format!("stamp-fresh-{suffix}"));

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP TABLE IF EXISTS {table} CASCADE"
    )))
    .execute(&pool)
    .await
    .expect("pre-clean");
    sqlx::query("DELETE FROM extension_migrations WHERE extension_id = $1")
        .bind(ext_id)
        .execute(&pool)
        .await
        .expect("pre-clean bookkeeping");

    let _cleanup = Cleanup {
        pool: pool.clone(),
        tables: vec![table],
        extension_ids: vec![ext_id],
    };

    let schema_sql: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY, payload JSONB);"
    ));
    let migration_sql: &'static str =
        leak_str("SELECT 1/0 AS this_must_never_execute_on_a_fresh_database;".to_string());

    let ext = CanaryExtension {
        id: ext_id,
        schema_sql,
        table,
        migration_sql,
    };

    let db_arc = Arc::new(db);
    let mut registry = ExtensionRegistry::new();
    registry.register(Arc::new(ext)).expect("register");

    install_extension_schemas(&registry, db_arc.as_ref())
        .await
        .expect("fresh install must stamp the canary migration, never execute it");

    assert_eq!(
        applied_versions(&pool, ext_id).await,
        vec![1],
        "the canary migration must be recorded as applied"
    );
    assert!(
        column_exists(&pool, table, "payload").await,
        "declarative schema must have materialised the target state"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn established_database_without_tracking_rows_executes_migrations() {
    let url = database_url();
    let db = Database::new_postgres(&url)
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak_str(format!("stamp_legacy_{suffix}"));
    let ext_id: &'static str = leak_str(format!("stamp-legacy-{suffix}"));

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP TABLE IF EXISTS {table} CASCADE"
    )))
    .execute(&pool)
    .await
    .expect("pre-clean");
    sqlx::query("DELETE FROM extension_migrations WHERE extension_id = $1")
        .bind(ext_id)
        .execute(&pool)
        .await
        .expect("pre-clean bookkeeping");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE {table} (id TEXT PRIMARY KEY)"
    )))
    .execute(&pool)
    .await
    .expect("create legacy table with no tracking rows");

    let _cleanup = Cleanup {
        pool: pool.clone(),
        tables: vec![table],
        extension_ids: vec![ext_id],
    };

    let schema_sql: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY, payload JSONB);"
    ));
    let migration_sql: &'static str = leak_str(format!(
        "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS payload JSONB; ALTER TABLE {table} ADD \
         COLUMN IF NOT EXISTS migrated_marker BOOLEAN;"
    ));

    let ext = CanaryExtension {
        id: ext_id,
        schema_sql,
        table,
        migration_sql,
    };

    let db_arc = Arc::new(db);
    let mut registry = ExtensionRegistry::new();
    registry.register(Arc::new(ext)).expect("register");

    install_extension_schemas(&registry, db_arc.as_ref())
        .await
        .expect("legacy install must execute the migration");

    assert_eq!(
        applied_versions(&pool, ext_id).await,
        vec![1],
        "the migration must be recorded"
    );
    assert!(
        column_exists(&pool, table, "payload").await,
        "the migration must have added the column to the legacy table"
    );
    assert!(
        column_exists(&pool, table, "migrated_marker").await,
        "the marker column proves the migration executed rather than being stamped"
    );
}
