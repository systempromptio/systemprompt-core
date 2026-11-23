//! Invariant under test: a migration may reference a function only a
//! declarative schema defines — the routine pre-pass applies every
//! declarative function before any migration runs, whatever the registry
//! order — while a migration that names a declarative-only trigger is
//! refused before any statement runs unless the reference is guarded inside
//! a `DO $$ … $$` block.

use std::sync::Arc;

use sqlx::{PgPool, Row};
use systemprompt_database::Database;
use systemprompt_extension::{
    ContributionError, Contributions, Extension, ExtensionMetadata, LoaderError, Migration,
    SchemaDefinition,
};
use systemprompt_test_fixtures::DisposableDb;

use crate::composition::install;
use uuid::Uuid;

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
    migration_sql: Option<&'static str>,
}

impl CanaryExtension {
    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::new(
            self.table.to_string(),
            self.schema_sql.to_string(),
        )]
    }

    fn migrations(&self) -> Vec<Migration> {
        self.migration_sql
            .map(|sql| vec![Migration::new(1, "canary", sql)])
            .unwrap_or_default()
    }
}

impl Extension for CanaryExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "canary-test",
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

struct Fixture {
    database: DisposableDb,
    pool: PgPool,
    db: Arc<Database>,
    suffix: String,
}

async fn connect() -> Fixture {
    let database = DisposableDb::with_schema("declarative_object_refs").await;
    let db = Database::new_postgres(database.url())
        .await
        .expect("connect to the disposable database");
    let pool: PgPool = db.pool().as_ref().clone();
    Fixture {
        database,
        pool,
        db: Arc::new(db),
        suffix: fresh_suffix(),
    }
}

impl Fixture {
    async fn finish(self) {
        self.pool.close().await;
        drop(self.db);
        self.database.drop_now().await;
    }
}

async fn trigger_exists(pool: &PgPool, name: &str) -> bool {
    sqlx::query("SELECT 1 AS one FROM pg_trigger WHERE tgname = $1")
        .bind(name)
        .fetch_optional(pool)
        .await
        .expect("trigger lookup must succeed")
        .is_some()
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

// Why: extension A declares the function; extension B is established (its
// table exists, no tracking rows) and its migration creates a trigger on it.
// Before the routine pre-pass this failed with "function does not exist" on
// every database that had never booted on A's schema.
async fn run_cross_extension_function_case(function_owner_first: bool) {
    let fx = connect().await;
    let s = &fx.suffix;
    let a_table: &'static str = leak_str(format!("routine_owner_{s}"));
    let b_table: &'static str = leak_str(format!("routine_user_{s}"));
    let function: &'static str = leak_str(format!("routine_fn_{s}"));
    let trigger: &'static str = leak_str(format!("routine_trg_{s}"));
    let a_id: &'static str = leak_str(format!("routine-owner-{s}"));
    let b_id: &'static str = leak_str(format!("routine-user-{s}"));

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE {b_table} (id TEXT PRIMARY KEY)"
    )))
    .execute(&fx.pool)
    .await
    .expect("create established table with no tracking rows");

    let owner = CanaryExtension {
        id: a_id,
        schema_sql: leak_str(format!(
            "CREATE TABLE IF NOT EXISTS {a_table} (id TEXT PRIMARY KEY);\n\
             CREATE OR REPLACE FUNCTION {function}() RETURNS trigger LANGUAGE plpgsql AS $$ \
             BEGIN RETURN NEW; END $$;"
        )),
        table: a_table,
        migration_sql: None,
    };
    let user = CanaryExtension {
        id: b_id,
        schema_sql: leak_str(format!(
            "CREATE TABLE IF NOT EXISTS {b_table} (id TEXT PRIMARY KEY);"
        )),
        table: b_table,
        migration_sql: Some(leak_str(format!(
            "CREATE TRIGGER {trigger} BEFORE INSERT ON {b_table} FOR EACH ROW EXECUTE FUNCTION \
             {function}();"
        ))),
    };

    let members: Vec<Box<dyn Extension>> = if function_owner_first {
        vec![Box::new(owner), Box::new(user)]
    } else {
        vec![Box::new(user), Box::new(owner)]
    };

    install(members, fx.db.as_ref())
        .await
        .expect("a migration may execute a function only the declarative schema defines");

    assert!(
        trigger_exists(&fx.pool, trigger).await,
        "the migration's trigger must exist"
    );
    assert_eq!(applied_versions(&fx.pool, b_id).await, vec![1]);
    fx.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn migration_trigger_can_reference_function_declared_by_another_extension() {
    run_cross_extension_function_case(true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn registry_order_does_not_matter_for_declarative_functions() {
    run_cross_extension_function_case(false).await;
}

fn trigger_schema(table: &str, function: &str, trigger: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);\n\
         CREATE OR REPLACE FUNCTION {function}() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN RETURN NEW; END $$;\n\
         CREATE TRIGGER {trigger} BEFORE UPDATE ON {table} FOR EACH ROW EXECUTE FUNCTION \
         {function}();"
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn migration_toggling_a_declarative_trigger_by_name_is_refused_before_any_statement_runs() {
    let fx = connect().await;
    let s = &fx.suffix;
    let table: &'static str = leak_str(format!("refs_refused_{s}"));
    let function: &'static str = leak_str(format!("refs_refused_fn_{s}"));
    let trigger: &'static str = leak_str(format!("refs_refused_trg_{s}"));
    let ext_id: &'static str = leak_str(format!("refs-refused-{s}"));

    let ext = CanaryExtension {
        id: ext_id,
        schema_sql: leak_str(trigger_schema(table, function, trigger)),
        table,
        migration_sql: Some(leak_str(format!(
            "ALTER TABLE {table} DISABLE TRIGGER {trigger};"
        ))),
    };
    let err = install(vec![Box::new(ext)], fx.db.as_ref())
        .await
        .expect_err("a bare reference to a declarative-only trigger must be refused");

    match &err {
        LoaderError::MigrationTogglesTriggerByName {
            extension,
            migration,
            table: toggled,
            trigger: named,
        } => {
            assert_eq!(extension, ext_id);
            assert_eq!(migration, "001_canary");
            assert_eq!(toggled, table);
            assert_eq!(named, trigger);
        },
        other => panic!("expected MigrationTogglesTriggerByName, got {other:?}"),
    }
    assert!(
        !table_exists(&fx.pool, table).await,
        "refusal must happen before the structural phase writes anything"
    );
    fx.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_do_block_guarding_the_same_trigger_is_accepted() {
    let fx = connect().await;
    let s = &fx.suffix;
    let table: &'static str = leak_str(format!("refs_guarded_{s}"));
    let function: &'static str = leak_str(format!("refs_guarded_fn_{s}"));
    let trigger: &'static str = leak_str(format!("refs_guarded_trg_{s}"));
    let ext_id: &'static str = leak_str(format!("refs-guarded-{s}"));

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE {table} (id TEXT PRIMARY KEY)"
    )))
    .execute(&fx.pool)
    .await
    .expect("create established table that never had the trigger");

    let ext = CanaryExtension {
        id: ext_id,
        schema_sql: leak_str(trigger_schema(table, function, trigger)),
        table,
        migration_sql: Some(leak_str(format!(
            "DO $$ BEGIN IF EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = '{trigger}') THEN \
             EXECUTE 'ALTER TABLE {table} DISABLE TRIGGER {trigger}'; END IF; END $$;"
        ))),
    };
    install(vec![Box::new(ext)], fx.db.as_ref())
        .await
        .expect("a catalog-guarded reference inside a DO block is the sanctioned form");

    assert_eq!(applied_versions(&fx.pool, ext_id).await, vec![1]);
    assert!(
        trigger_exists(&fx.pool, trigger).await,
        "the dependent phase must still create the trigger afterwards"
    );
    fx.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_function_whose_signature_a_migration_reshapes_is_left_to_that_migration() {
    let fx = connect().await;
    let s = &fx.suffix;
    let table: &'static str = leak_str(format!("reshape_{s}"));
    let function: &'static str = leak_str(format!("reshape_fn_{s}"));
    let ext_id: &'static str = leak_str(format!("reshape-{s}"));

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE {table} (id TEXT PRIMARY KEY)"
    )))
    .execute(&fx.pool)
    .await
    .expect("create established table");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE FUNCTION {function}() RETURNS integer LANGUAGE sql AS 'SELECT 1'"
    )))
    .execute(&fx.pool)
    .await
    .expect("create the old shape of the function");

    let ext = CanaryExtension {
        id: ext_id,
        schema_sql: leak_str(format!(
            "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);\n\
             CREATE OR REPLACE FUNCTION {function}() RETURNS text LANGUAGE sql AS 'SELECT ''new''';"
        )),
        table,
        migration_sql: Some(leak_str(format!("DROP FUNCTION {function}();"))),
    };
    install(vec![Box::new(ext)], fx.db.as_ref())
        .await
        .expect("the pre-pass must not fail on a signature only a migration can change");

    let return_type: String = sqlx::query_scalar(
        "SELECT pg_get_function_result(p.oid) FROM pg_proc p WHERE p.proname = $1",
    )
    .bind(function)
    .fetch_one(&fx.pool)
    .await
    .expect("function present after install");
    assert_eq!(return_type, "text");
    fx.finish().await;
}

// Why: a table an extension created in one migration and dropped in a later
// one is nowhere in schemas(); the ALTER between those two must still be
// recognised as the extension's own when the chain replays on an old database.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_migration_may_alter_a_table_an_earlier_migration_of_its_own_created() {
    let fx = connect().await;
    let s = &fx.suffix;
    let table: &'static str = leak_str(format!("own_{s}"));
    let transient: &'static str = leak_str(format!("own_transient_{s}"));
    let ext_id: &'static str = leak_str(format!("own-{s}"));

    sqlx::query(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE {table} (id TEXT PRIMARY KEY)"
    )))
    .execute(&fx.pool)
    .await
    .expect("create established table");

    struct ChainExtension {
        id: &'static str,
        table: &'static str,
        transient: &'static str,
    }
    impl ChainExtension {
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
            vec![
                Migration::new(
                    1,
                    "create",
                    leak_str(format!(
                        "CREATE TABLE IF NOT EXISTS {} (id TEXT PRIMARY KEY);",
                        self.transient
                    )),
                ),
                Migration::new(
                    2,
                    "alter",
                    leak_str(format!(
                        "ALTER TABLE {} ADD COLUMN IF NOT EXISTS note TEXT;",
                        self.transient
                    )),
                ),
                Migration::new(
                    3,
                    "drop",
                    leak_str(format!("DROP TABLE IF EXISTS {};", self.transient)),
                ),
            ]
        }
    }

    impl Extension for ChainExtension {
        fn metadata(&self) -> ExtensionMetadata {
            ExtensionMetadata {
                id: self.id,
                name: "chain-test",
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

    install(
        vec![Box::new(ChainExtension {
            id: ext_id,
            table,
            transient,
        })],
        fx.db.as_ref(),
    )
    .await
    .expect("an ALTER on a table an earlier migration created is the extension's own");
    assert_eq!(applied_versions(&fx.pool, ext_id).await, vec![1, 2, 3]);
    assert!(!table_exists(&fx.pool, transient).await);
    fx.finish().await;
}
