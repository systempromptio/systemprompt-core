//! Invariant under test: an install that fails part-way leaves no extension
//! holding its tables without the baseline that claims them.
//!
//! Freshness is decided before any DDL runs — no tracking rows, none of the
//! extension's owned tables. If the structural DDL commits and the baseline
//! stamp does not, that verdict is destroyed: the tables now exist, so the
//! next install calls the extension established and executes migration SQL
//! the declarative schema has already superseded. Re-running never recovers.

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

/// An extension whose schema and single migration are supplied per-test.
struct StampExt {
    id: &'static str,
    table: &'static str,
    schema_sql: &'static str,
    migration_sql: &'static str,
}

impl Extension for StampExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "interrupted-install-test",
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_install_never_leaves_a_stamped_extension_unstamped() {
    let url = database_url();
    let db = Database::new_postgres(&url)
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();

    let suffix = Uuid::new_v4().simple().to_string()[..12].to_string();
    let good_table: &'static str = leak_str(format!("interrupted_aaa_{suffix}"));
    let bad_table: &'static str = leak_str(format!("interrupted_zzz_{suffix}"));
    // Why: the registry installs extensions in sorted order, so the ids decide
    // which one commits its tables before the other one fails.
    let good_id: &'static str = leak_str(format!("interrupted-aaa-{suffix}"));
    let bad_id: &'static str = leak_str(format!("interrupted-zzz-{suffix}"));

    let _cleanup = Cleanup {
        pool: pool.clone(),
        tables: vec![good_table, bad_table],
        extension_ids: vec![good_id, bad_id],
    };

    let good_schema: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {good_table} (id TEXT PRIMARY KEY, payload JSONB);"
    ));
    // Why: standing in for every migration whose SQL only makes sense against
    // the schema shape of its own era. Executing it on a database built from
    // today's declarative schema is the failure this test exists for, and the
    // divide-by-zero makes that unambiguous.
    let canary_sql: &'static str = leak_str(
        "SELECT 1 / 0 AS this_must_never_execute_on_a_database_that_was_fresh;".to_string(),
    );
    // Why: parses and lints as an ordinary `CREATE TABLE`, so it reaches the
    // structural phase and fails there on the absent referenced relation — an
    // install interrupted after an earlier extension has already committed its
    // own tables.
    let bad_schema: &'static str = leak_str(format!(
        "CREATE TABLE IF NOT EXISTS {bad_table} (id TEXT PRIMARY KEY, ref TEXT REFERENCES \
         no_such_table_{suffix}(id));"
    ));

    let db_arc = Arc::new(db);
    let mut registry = ExtensionRegistry::new();
    registry
        .register(Arc::new(StampExt {
            id: good_id,
            table: good_table,
            schema_sql: good_schema,
            migration_sql: canary_sql,
        }))
        .expect("register the extension installed first");
    registry
        .register(Arc::new(StampExt {
            id: bad_id,
            table: bad_table,
            schema_sql: bad_schema,
            migration_sql: "SELECT 1;",
        }))
        .expect("register the extension whose DDL fails");

    let failed = install_extension_schemas(&registry, db_arc.as_ref()).await;
    let failure = failed.expect_err("a reference to an absent relation must fail the install");
    assert!(
        format!("{failure}").contains(bad_table),
        "the install must fail on the second extension's DDL, after the first has committed its \
         tables: {failure}"
    );

    assert!(
        table_exists(&pool, good_table).await,
        "the install must have failed after the first extension committed its tables"
    );
    assert_eq!(
        applied_versions(&pool, good_id).await,
        vec![1],
        "an extension whose tables are committed must carry its baseline too — without it \
             the next install calls the database established and runs migration SQL for a schema \
         shape it never had"
    );

    let mut retry = ExtensionRegistry::new();
    retry
        .register(Arc::new(StampExt {
            id: good_id,
            table: good_table,
            schema_sql: good_schema,
            migration_sql: canary_sql,
        }))
        .expect("register for the retry");

    install_extension_schemas(&retry, db_arc.as_ref())
        .await
        .expect("re-installing after an interrupted install must not execute migration SQL");

    assert_eq!(
        applied_versions(&pool, good_id).await,
        vec![1],
        "the retry must neither duplicate nor drop the baseline row"
    );
}
