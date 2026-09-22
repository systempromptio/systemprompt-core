//! Invariant under test: a fresh extension's retirement migration — every
//! statement a `DROP … IF EXISTS` or a `DELETE FROM extension_migrations` —
//! is stamped like the rest of its chain *and executed*, so a relation left
//! behind by a since-deleted extension is dropped on the first boot that
//! carries the drop, not kept because the dropping extension was new. A
//! migration that is not a retirement is still never executed on a fresh
//! database.

use std::env;
use std::sync::Arc;

use sqlx::{PgPool, Row};
use systemprompt_database::{Database, install_extension_schemas, is_retirement};
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

struct RetiringExtension {
    id: &'static str,
    schema_sql: &'static str,
    table: &'static str,
    retirement_sql: &'static str,
}

impl Extension for RetiringExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "retiring-test",
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
            Migration::new(
                1,
                "never_runs",
                "SELECT 1/0 AS this_must_never_execute_on_a_fresh_database;",
            ),
            Migration::new(2, "retire_orphans", self.retirement_sql),
        ]
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

async fn table_exists(pool: &PgPool, table: &str) -> bool {
    sqlx::query(
        "SELECT 1 AS one FROM information_schema.tables WHERE table_schema = 'public' AND \
         table_name = $1",
    )
    .bind(table)
    .fetch_optional(pool)
    .await
    .expect("table lookup must succeed")
    .is_some()
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

#[test]
fn retirement_is_only_idempotent_drops_and_ledger_deletes() {
    let retire = |sql: &'static str| is_retirement(&Migration::new(1, "m", sql));
    assert!(retire(
        "DROP TABLE IF EXISTS eval_runs CASCADE; DROP VIEW IF EXISTS eval_summary; \
         DELETE FROM extension_migrations WHERE extension_id = 'evaluation';"
    ));
    assert!(retire(
        "DROP FUNCTION IF EXISTS gone(); DROP TRIGGER IF EXISTS t ON x;"
    ));
    assert!(
        !retire("DROP TABLE eval_runs;"),
        "a drop without IF EXISTS is not idempotent"
    );
    assert!(
        !retire("DELETE FROM eval_runs;"),
        "only the ledger may be deleted from"
    );
    assert!(
        !retire("DROP TABLE IF EXISTS a; ALTER TABLE b ADD COLUMN c TEXT;"),
        "any non-retiring statement disqualifies the migration"
    );
    assert!(!retire(""), "an empty body retires nothing");
    assert!(!retire("this is not sql"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_extension_executes_its_retirement_migration() {
    let url = database_url();
    let db = Database::new_postgres(&url)
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();

    let suffix = fresh_suffix();
    let table: &'static str = leak_str(format!("retire_own_{suffix}"));
    let orphan: &'static str = leak_str(format!("retire_orphan_{suffix}"));
    let ext_id: &'static str = leak_str(format!("retire-{suffix}"));
    let gone_ext: &'static str = leak_str(format!("gone-{suffix}"));

    for t in [table, orphan] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP TABLE IF EXISTS {t} CASCADE"
        )))
        .execute(&pool)
        .await
        .expect("pre-clean");
    }
    let _cleanup = Cleanup {
        pool: pool.clone(),
        tables: vec![table, orphan],
        extension_ids: vec![ext_id, gone_ext],
    };

    // What a deleted extension leaves behind: its table and its ledger row.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE {orphan} (id TEXT PRIMARY KEY)"
    )))
    .execute(&pool)
    .await
    .expect("seed orphan table");
    sqlx::query(
        "INSERT INTO extension_migrations (id, extension_id, version, name, checksum) \
         VALUES ($1, $2, 1, 'legacy', 'x')",
    )
    .bind(format!("{gone_ext}_001"))
    .bind(gone_ext)
    .execute(&pool)
    .await
    .expect("seed orphan ledger row");

    let schema_sql: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);"
    ));
    let retirement_sql: &'static str = leak_str(format!(
        "DROP TABLE IF EXISTS {orphan} CASCADE;\n\
         DELETE FROM extension_migrations WHERE extension_id = '{gone_ext}';"
    ));
    let ext = RetiringExtension {
        id: ext_id,
        schema_sql,
        table,
        retirement_sql,
    };

    let db_arc = Arc::new(db);
    let mut registry = ExtensionRegistry::new();
    registry.register(Arc::new(ext)).expect("register");

    install_extension_schemas(&registry, db_arc.as_ref())
        .await
        .expect("fresh install must stamp both migrations and execute only the retirement");

    assert_eq!(
        applied_versions(&pool, ext_id).await,
        vec![1, 2],
        "both migrations are recorded as applied"
    );
    assert!(
        !table_exists(&pool, orphan).await,
        "the retirement must have dropped the orphaned table"
    );
    assert!(
        applied_versions(&pool, gone_ext).await.is_empty(),
        "the retirement must have removed the deleted extension's ledger rows"
    );
    assert!(table_exists(&pool, table).await);
}
