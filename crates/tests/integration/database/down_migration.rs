//! Invariant under test: `MigrationService::run_down_migrations` rolls back
//! the most recently applied migration using the declared `down` SQL,
//! removes the bookkeeping row, and leaves the extension in a state where
//! `run_pending_migrations` re-applies it cleanly. Also covers the
//! "missing down" rejection path.

use std::env;
use std::sync::Arc;

use sqlx::{PgPool, Row};
use systemprompt_database::{Database, MigrationService};
use systemprompt_extension::{
    Extension, ExtensionMetadata, LoaderError, Migration, SchemaDefinition,
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

struct ReversibleExtension {
    id: &'static str,
    up_sql: &'static str,
    down_sql: &'static str,
}

impl Extension for ReversibleExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "reversible-test",
            version: "0.0.0",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![]
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![Migration::with_down(
            1,
            "create_demo_table",
            self.up_sql,
            self.down_sql,
        )]
    }
}

struct IrreversibleExtension {
    id: &'static str,
    up_sql: &'static str,
}

impl Extension for IrreversibleExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "irreversible-test",
            version: "0.0.0",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![]
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![Migration::new(1, "no_down", self.up_sql)]
    }
}

async fn table_exists(pool: &PgPool, table: &str) -> bool {
    let row = sqlx::query(
        "SELECT 1 AS one FROM information_schema.tables WHERE table_schema = 'public' AND \
         table_name = $1",
    )
    .bind(table)
    .fetch_optional(pool)
    .await
    .expect("table lookup must succeed");
    row.is_some()
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_revert_reapply_round_trip() {
    let url = database_url();
    let db = Database::new_postgres(&url)
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak_str(format!("revert_demo_{suffix}"));
    let ext_id: &'static str = leak_str(format!("revert-ext-{suffix}"));
    let up_sql: &'static str = leak_str(format!(
        "CREATE TABLE {table} (id TEXT PRIMARY KEY, payload TEXT);"
    ));
    let down_sql: &'static str = leak_str(format!("DROP TABLE {table};"));

    sqlx::query(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
        .execute(&pool)
        .await
        .expect("pre-clean");
    sqlx::query("DELETE FROM extension_migrations WHERE extension_id = $1")
        .bind(ext_id)
        .execute(&pool)
        .await
        .expect("pre-clean bookkeeping");

    let ext = ReversibleExtension {
        id: ext_id,
        up_sql,
        down_sql,
    };

    let db_arc = Arc::new(db);
    let provider = db_arc.write();
    let service = MigrationService::new(provider);

    let applied = service
        .run_pending_migrations(&ext)
        .await
        .expect("apply succeeds");
    assert_eq!(applied.migrations_run, 1);
    assert!(table_exists(&pool, table).await);
    assert_eq!(applied_versions(&pool, ext_id).await, vec![1]);

    let reverted = service
        .run_down_migrations(&ext, 1)
        .await
        .expect("revert succeeds");
    assert_eq!(reverted.migrations_run, 1);
    assert!(!table_exists(&pool, table).await);
    assert!(applied_versions(&pool, ext_id).await.is_empty());

    let reapplied = service
        .run_pending_migrations(&ext)
        .await
        .expect("re-apply succeeds");
    assert_eq!(reapplied.migrations_run, 1);
    assert!(table_exists(&pool, table).await);
    assert_eq!(applied_versions(&pool, ext_id).await, vec![1]);

    sqlx::query(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
        .execute(&pool)
        .await
        .expect("cleanup table");
    sqlx::query("DELETE FROM extension_migrations WHERE extension_id = $1")
        .bind(ext_id)
        .execute(&pool)
        .await
        .expect("cleanup bookkeeping");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revert_rejects_irreversible_migration() {
    let url = database_url();
    let db = Database::new_postgres(&url)
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak_str(format!("noundo_demo_{suffix}"));
    let ext_id: &'static str = leak_str(format!("noundo-ext-{suffix}"));
    let up_sql: &'static str = leak_str(format!("CREATE TABLE {table} (id TEXT PRIMARY KEY);"));

    sqlx::query(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
        .execute(&pool)
        .await
        .expect("pre-clean");
    sqlx::query("DELETE FROM extension_migrations WHERE extension_id = $1")
        .bind(ext_id)
        .execute(&pool)
        .await
        .expect("pre-clean bookkeeping");

    let ext = IrreversibleExtension { id: ext_id, up_sql };

    let db_arc = Arc::new(db);
    let provider = db_arc.write();
    let service = MigrationService::new(provider);

    service
        .run_pending_migrations(&ext)
        .await
        .expect("apply succeeds");

    let err = service
        .run_down_migrations(&ext, 1)
        .await
        .expect_err("revert must reject irreversible migration");
    assert!(matches!(
        err,
        LoaderError::MigrationNotReversible { version: 1, .. }
    ));

    assert_eq!(
        applied_versions(&pool, ext_id).await,
        vec![1],
        "rejection must leave bookkeeping untouched"
    );

    sqlx::query(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
        .execute(&pool)
        .await
        .expect("cleanup table");
    sqlx::query("DELETE FROM extension_migrations WHERE extension_id = $1")
        .bind(ext_id)
        .execute(&pool)
        .await
        .expect("cleanup bookkeeping");
}
