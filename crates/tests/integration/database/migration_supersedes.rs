//! Invariant under test: a migration declaring `@supersedes-checksum` (the
//! checksum of the text it replaces) moves a tracking row holding that
//! checksum to its own without executing anything, and a database that
//! never applied either text runs the new one.

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

struct CanaryExtension {
    id: &'static str,
    table: &'static str,
    migration: Migration,
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
            format!(
                "CREATE TABLE IF NOT EXISTS {} (id TEXT PRIMARY KEY);",
                self.table
            ),
        )]
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![self.migration.clone()]
    }
}

async fn stored_checksum(pool: &PgPool, ext_id: &str) -> Option<String> {
    sqlx::query("SELECT checksum FROM extension_migrations WHERE extension_id = $1 AND version = 1")
        .bind(ext_id)
        .fetch_optional(pool)
        .await
        .expect("query checksum")
        .map(|r| r.get::<String, _>("checksum"))
}

async fn install(db: &Arc<Database>, ext: CanaryExtension) {
    let mut registry = ExtensionRegistry::new();
    registry.register(Arc::new(ext)).expect("register");
    install_extension_schemas(&registry, db.as_ref())
        .await
        .expect("install");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_superseding_migration_moves_the_applied_row_without_executing() {
    let db = Arc::new(
        Database::new_postgres(&database_url())
            .await
            .expect("connect to test postgres"),
    );
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();
    let suffix = Uuid::new_v4().simple().to_string()[..12].to_string();
    let table: &'static str = leak_str(format!("supersede_{suffix}"));
    let ext_id: &'static str = leak_str(format!("supersede-{suffix}"));

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE {table} (id TEXT PRIMARY KEY)"
    )))
    .execute(&pool)
    .await
    .expect("established table");

    let old_sql: &'static str = leak_str(format!(
        "ALTER TABLE {table} ADD COLUMN IF NOT EXISTS old_marker BOOLEAN;"
    ));
    let old = Migration::new(1, "shape", old_sql);
    let old_checksum = leak_str(old.checksum());
    install(
        &db,
        CanaryExtension {
            id: ext_id,
            table,
            migration: old,
        },
    )
    .await;
    assert_eq!(
        stored_checksum(&pool, ext_id).await.as_deref(),
        Some(old_checksum)
    );

    let new_sql: &'static str =
        leak_str("SELECT 1/0 AS the_superseding_text_must_not_run_here;".to_owned());
    let new = Migration::new(1, "shape", new_sql).superseding(old_checksum);
    let new_checksum = new.checksum();
    install(
        &db,
        CanaryExtension {
            id: ext_id,
            table,
            migration: new,
        },
    )
    .await;
    assert_eq!(
        stored_checksum(&pool, ext_id).await,
        Some(new_checksum),
        "the tracking row must move to the superseding checksum"
    );

    let _ = sqlx::query(sqlx::AssertSqlSafe(format!("DROP TABLE IF EXISTS {table}")))
        .execute(&pool)
        .await;
    let _ = sqlx::query("DELETE FROM extension_migrations WHERE extension_id = $1")
        .bind(ext_id)
        .execute(&pool)
        .await;
}
