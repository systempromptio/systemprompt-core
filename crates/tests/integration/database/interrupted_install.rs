//! Invariant under test: an install that fails part-way leaves no extension
//! holding its tables without the baseline that claims them.
//!
//! Freshness is decided before any DDL runs — no tracking rows, none of the
//! extension's owned tables. If the structural DDL commits and the baseline
//! stamp does not, that verdict is destroyed: the tables now exist, so the
//! next install calls the extension established and executes migration SQL
//! the declarative schema has already superseded. Re-running never recovers.

use std::sync::Arc;

use sqlx::{PgPool, Row};
use systemprompt_database::Database;
use systemprompt_extension::{
    ContributionError, Contributions, Extension, ExtensionMetadata, Migration, SchemaDefinition,
};
use systemprompt_test_fixtures::DisposableDb;

use crate::composition::install;
use uuid::Uuid;

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

struct StampExt {
    id: &'static str,
    table: &'static str,
    schema_sql: &'static str,
    migration_sql: &'static str,
}

impl StampExt {
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

impl Extension for StampExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "interrupted-install-test",
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
    let database = DisposableDb::with_schema("interrupted_install").await;
    let db = Database::new_postgres(database.url())
        .await
        .expect("connect to the disposable database");
    let pool: PgPool = db.pool().as_ref().clone();

    let suffix = Uuid::new_v4().simple().to_string()[..12].to_string();
    let good_table: &'static str = leak_str(format!("interrupted_aaa_{suffix}"));
    let bad_table: &'static str = leak_str(format!("interrupted_zzz_{suffix}"));
    let good_id: &'static str = leak_str(format!("interrupted-aaa-{suffix}"));
    let bad_id: &'static str = leak_str(format!("interrupted-zzz-{suffix}"));

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
    let failed = install(
        vec![
            Box::new(StampExt {
                id: good_id,
                table: good_table,
                schema_sql: good_schema,
                migration_sql: canary_sql,
            }),
            Box::new(StampExt {
                id: bad_id,
                table: bad_table,
                schema_sql: bad_schema,
                migration_sql: "SELECT 1;",
            }),
        ],
        db_arc.as_ref(),
    )
    .await;
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

    install(
        vec![Box::new(StampExt {
            id: good_id,
            table: good_table,
            schema_sql: good_schema,
            migration_sql: canary_sql,
        })],
        db_arc.as_ref(),
    )
    .await
    .expect("re-installing after an interrupted install must not execute migration SQL");

    assert_eq!(
        applied_versions(&pool, good_id).await,
        vec![1],
        "the retry must neither duplicate nor drop the baseline row"
    );
    pool.close().await;
    drop(db_arc);
    database.drop_now().await;
}
