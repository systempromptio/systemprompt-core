//! Invariant under test: install is globally phased — structural DDL
//! (`CREATE TABLE`/`TYPE`/`EXTENSION`), then `Extension::migrations()`, then
//! dependent DDL (`CREATE INDEX`/`VIEW`/etc.). A migration therefore runs
//! after a legacy table exists but before the index or view that references a
//! column the migration introduces, so a legacy database reaches the target
//! shape in the same boot — without tripping the schema linter that
//! hard-rejects imperative SQL in schemas.

use std::env;
use std::sync::Arc;

use sqlx::{PgPool, Row};
use systemprompt_database::{Database, install_extension_schemas};
use systemprompt_extension::{
    Extension, ExtensionMetadata, ExtensionRegistry, Migration, SchemaDefinition,
};
use uuid::Uuid;

const DEFAULT_DATABASE_URL: &str =
    "postgres://systemprompt_admin:3e00fcdac26b5b731829e8737515db8f@localhost:5432/systemprompt-web";

fn database_url() -> String {
    env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string())
}

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn fresh_suffix() -> String {
    Uuid::new_v4().simple().to_string()[..12].to_string()
}

struct FixtureCleanup {
    pool: PgPool,
    tables: Vec<&'static str>,
    views: Vec<&'static str>,
    extension_ids: Vec<&'static str>,
}

impl Drop for FixtureCleanup {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        let tables = self.tables.clone();
        let views = self.views.clone();
        let extension_ids = self.extension_ids.clone();
        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                for v in &views {
                    let _ = sqlx::query(&format!("DROP VIEW IF EXISTS {v} CASCADE"))
                        .execute(&pool)
                        .await;
                }
                for t in &tables {
                    let _ = sqlx::query(&format!("DROP TABLE IF EXISTS {t} CASCADE"))
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

struct LogsExtension {
    id: &'static str,
    schema_sql: &'static str,
    migration_sql: &'static str,
    table: &'static str,
}

impl Extension for LogsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "logs-test",
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
        vec![Migration::new(1, "add_gateway_conversation_id", self.migration_sql)]
    }

    fn owned_tables(&self) -> Vec<&'static str> {
        vec![self.table]
    }
}

struct ViewExtension {
    id: &'static str,
    schema_sql: &'static str,
    migration_sql: &'static str,
    table: &'static str,
}

impl Extension for ViewExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "view-test",
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
        vec![Migration::new(1, "add_payload_column", self.migration_sql)]
    }

    fn owned_tables(&self) -> Vec<&'static str> {
        vec![self.table]
    }
}

async fn column_exists(pool: &PgPool, table: &str, column: &str) -> bool {
    let row = sqlx::query(
        "SELECT 1 AS one FROM information_schema.columns \
         WHERE table_schema = 'public' AND table_name = $1 AND column_name = $2",
    )
    .bind(table)
    .bind(column)
    .fetch_optional(pool)
    .await
    .expect("column lookup must succeed");
    row.is_some()
}

async fn index_exists(pool: &PgPool, index: &str) -> bool {
    let row = sqlx::query(
        "SELECT 1 AS one FROM pg_indexes WHERE schemaname = 'public' AND indexname = $1",
    )
    .bind(index)
    .fetch_optional(pool)
    .await
    .expect("index lookup must succeed");
    row.is_some()
}

async fn view_definition(pool: &PgPool, view: &str) -> Option<String> {
    let row = sqlx::query(
        "SELECT pg_get_viewdef(c.oid, true) AS def \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = 'public' AND c.relname = $1 AND c.relkind IN ('v','m')",
    )
    .bind(view)
    .fetch_optional(pool)
    .await
    .expect("view lookup must succeed");
    row.map(|r| r.get::<String, _>("def"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn migration_runs_before_schema_so_legacy_logs_get_new_column() {
    let url = database_url();
    let db = Database::new_postgres(&url)
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak_str(format!("logs_test_{suffix}"));
    let index_name: &'static str = leak_str(format!("idx_{table}_gateway_conversation_id"));
    let ext_id: &'static str = leak_str(format!("logs-ext-{suffix}"));

    sqlx::query(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
        .execute(&pool)
        .await
        .expect("pre-clean");
    sqlx::query(&format!(
        "CREATE TABLE {table} (id TEXT PRIMARY KEY, level TEXT, message TEXT)"
    ))
    .execute(&pool)
    .await
    .expect("create legacy logs table");

    let _cleanup = FixtureCleanup {
        pool: pool.clone(),
        tables: vec![table],
        views: vec![],
        extension_ids: vec![ext_id],
    };

    let schema_sql: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (\n    \
            id TEXT PRIMARY KEY,\n    \
            level TEXT,\n    \
            message TEXT,\n    \
            gateway_conversation_id VARCHAR(255)\n        \
        );\n\
        CREATE INDEX IF NOT EXISTS {index_name} \
            ON {table} (gateway_conversation_id);"
    ));
    let migration_sql: &'static str = leak_str(format!(
        "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS gateway_conversation_id VARCHAR(255);"
    ));

    let ext = LogsExtension {
        id: ext_id,
        schema_sql,
        migration_sql,
        table,
    };

    let db_arc = Arc::new(db);
    let mut registry = ExtensionRegistry::new();
    registry
        .register(Arc::new(ext))
        .expect("registry accepts extension");

    install_extension_schemas(&registry, db_arc.as_ref())
        .await
        .expect("install must succeed: migration adds column, then schema is a no-op");

    assert!(
        column_exists(&pool, table, "gateway_conversation_id").await,
        "migration must add gateway_conversation_id to legacy {table}"
    );
    assert!(
        index_exists(&pool, index_name).await,
        "schema must create {index_name} after the column exists"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn view_in_schema_can_reference_column_added_by_migration() {
    let url = database_url();
    let db = Database::new_postgres(&url)
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak_str(format!("events_test_{suffix}"));
    let view: &'static str = leak_str(format!("v_{table}"));
    let ext_id: &'static str = leak_str(format!("events-ext-{suffix}"));

    sqlx::query(&format!("DROP VIEW IF EXISTS {view} CASCADE"))
        .execute(&pool)
        .await
        .expect("pre-clean view");
    sqlx::query(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
        .execute(&pool)
        .await
        .expect("pre-clean table");
    sqlx::query(&format!(
        "CREATE TABLE {table} (id TEXT PRIMARY KEY, kind TEXT)"
    ))
    .execute(&pool)
    .await
    .expect("create legacy events table");

    let _cleanup = FixtureCleanup {
        pool: pool.clone(),
        tables: vec![table],
        views: vec![view],
        extension_ids: vec![ext_id],
    };

    let schema_sql: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (\n    \
            id TEXT PRIMARY KEY,\n    \
            kind TEXT,\n    \
            payload JSONB\n        \
        );\n\
        CREATE OR REPLACE VIEW {view} AS \
            SELECT id, payload FROM {table};"
    ));
    let migration_sql: &'static str = leak_str(format!(
        "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS payload JSONB;"
    ));

    let ext = ViewExtension {
        id: ext_id,
        schema_sql,
        migration_sql,
        table,
    };

    let db_arc = Arc::new(db);
    let mut registry = ExtensionRegistry::new();
    registry
        .register(Arc::new(ext))
        .expect("registry accepts extension");

    install_extension_schemas(&registry, db_arc.as_ref())
        .await
        .expect("install must succeed: migration adds payload, then view references it");

    assert!(
        column_exists(&pool, table, "payload").await,
        "migration must add payload column to legacy {table}"
    );
    let def = view_definition(&pool, view)
        .await
        .unwrap_or_else(|| panic!("view {view} must exist after schema install"));
    assert!(
        def.to_lowercase().contains("payload"),
        "view definition must reference the migration-added column; got: {def}"
    );
}
