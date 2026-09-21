//! Invariant under test: a migration may reference a function only a
//! declarative schema defines — the routine pre-pass applies every
//! declarative function before any migration runs, whatever the registry
//! order — while a migration that names a declarative-only trigger is
//! refused before any statement runs unless the reference is guarded inside
//! a `DO $$ … $$` block.

use std::env;
use std::sync::Arc;

use sqlx::{PgPool, Row};
use systemprompt_database::{Database, install_extension_schemas};
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

struct CanaryExtension {
    id: &'static str,
    schema_sql: &'static str,
    table: &'static str,
    migration_sql: Option<&'static str>,
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
        self.migration_sql
            .map(|sql| vec![Migration::new(1, "canary", sql)])
            .unwrap_or_default()
    }
}

struct Cleanup {
    pool: PgPool,
    tables: Vec<&'static str>,
    functions: Vec<&'static str>,
    extension_ids: Vec<&'static str>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        let tables = self.tables.clone();
        let functions = self.functions.clone();
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
                for f in &functions {
                    let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
                        "DROP FUNCTION IF EXISTS {f}() CASCADE"
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

struct Fixture {
    pool: PgPool,
    db: Arc<Database>,
    suffix: String,
}

async fn connect() -> Fixture {
    let db = Database::new_postgres(&database_url())
        .await
        .expect("connect to test postgres");
    let pool: PgPool = db.pool_arc().expect("pg pool").as_ref().clone();
    Fixture {
        pool,
        db: Arc::new(db),
        suffix: fresh_suffix(),
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

    let _cleanup = Cleanup {
        pool: fx.pool.clone(),
        tables: vec![a_table, b_table],
        functions: vec![function],
        extension_ids: vec![a_id, b_id],
    };

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

    let mut registry = ExtensionRegistry::new();
    if function_owner_first {
        registry.register(Arc::new(owner)).expect("register owner");
        registry.register(Arc::new(user)).expect("register user");
    } else {
        registry.register(Arc::new(user)).expect("register user");
        registry.register(Arc::new(owner)).expect("register owner");
    }

    install_extension_schemas(&registry, fx.db.as_ref())
        .await
        .expect("a migration may execute a function only the declarative schema defines");

    assert!(
        trigger_exists(&fx.pool, trigger).await,
        "the migration's trigger must exist"
    );
    assert_eq!(applied_versions(&fx.pool, b_id).await, vec![1]);
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
async fn migration_naming_a_declarative_only_trigger_is_refused_before_any_statement_runs() {
    let fx = connect().await;
    let s = &fx.suffix;
    let table: &'static str = leak_str(format!("refs_refused_{s}"));
    let function: &'static str = leak_str(format!("refs_refused_fn_{s}"));
    let trigger: &'static str = leak_str(format!("refs_refused_trg_{s}"));
    let ext_id: &'static str = leak_str(format!("refs-refused-{s}"));

    let _cleanup = Cleanup {
        pool: fx.pool.clone(),
        tables: vec![table],
        functions: vec![function],
        extension_ids: vec![ext_id],
    };

    let ext = CanaryExtension {
        id: ext_id,
        schema_sql: leak_str(trigger_schema(table, function, trigger)),
        table,
        migration_sql: Some(leak_str(format!(
            "ALTER TABLE {table} DISABLE TRIGGER {trigger};"
        ))),
    };
    let mut registry = ExtensionRegistry::new();
    registry.register(Arc::new(ext)).expect("register");

    let err = install_extension_schemas(&registry, fx.db.as_ref())
        .await
        .expect_err("a bare reference to a declarative-only trigger must be refused");

    match &err {
        LoaderError::MigrationReferencesDeclarativeObject {
            extension,
            migration,
            kind,
            object,
            ..
        } => {
            assert_eq!(extension, ext_id);
            assert_eq!(migration, "001_canary");
            assert_eq!(kind, "trigger");
            assert_eq!(object, trigger);
        },
        other => panic!("expected MigrationReferencesDeclarativeObject, got {other:?}"),
    }
    assert!(
        !table_exists(&fx.pool, table).await,
        "refusal must happen before the structural phase writes anything"
    );
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

    let _cleanup = Cleanup {
        pool: fx.pool.clone(),
        tables: vec![table],
        functions: vec![function],
        extension_ids: vec![ext_id],
    };

    let ext = CanaryExtension {
        id: ext_id,
        schema_sql: leak_str(trigger_schema(table, function, trigger)),
        table,
        migration_sql: Some(leak_str(format!(
            "DO $$ BEGIN IF EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = '{trigger}') THEN \
             EXECUTE 'ALTER TABLE {table} DISABLE TRIGGER {trigger}'; END IF; END $$;"
        ))),
    };
    let mut registry = ExtensionRegistry::new();
    registry.register(Arc::new(ext)).expect("register");

    install_extension_schemas(&registry, fx.db.as_ref())
        .await
        .expect("a catalog-guarded reference inside a DO block is the sanctioned form");

    assert_eq!(applied_versions(&fx.pool, ext_id).await, vec![1]);
    assert!(
        trigger_exists(&fx.pool, trigger).await,
        "the dependent phase must still create the trigger afterwards"
    );
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

    let _cleanup = Cleanup {
        pool: fx.pool.clone(),
        tables: vec![table],
        functions: vec![function],
        extension_ids: vec![ext_id],
    };

    let ext = CanaryExtension {
        id: ext_id,
        schema_sql: leak_str(format!(
            "CREATE TABLE IF NOT EXISTS {table} (id TEXT PRIMARY KEY);\n\
             CREATE OR REPLACE FUNCTION {function}() RETURNS text LANGUAGE sql AS 'SELECT ''new''';"
        )),
        table,
        migration_sql: Some(leak_str(format!("DROP FUNCTION {function}();"))),
    };
    let mut registry = ExtensionRegistry::new();
    registry.register(Arc::new(ext)).expect("register");

    install_extension_schemas(&registry, fx.db.as_ref())
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

    let _cleanup = Cleanup {
        pool: fx.pool.clone(),
        tables: vec![table, transient],
        functions: vec![],
        extension_ids: vec![ext_id],
    };

    struct ChainExtension {
        id: &'static str,
        table: &'static str,
        transient: &'static str,
    }
    impl Extension for ChainExtension {
        fn metadata(&self) -> ExtensionMetadata {
            ExtensionMetadata {
                id: self.id,
                name: "chain-test",
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

    let mut registry = ExtensionRegistry::new();
    registry
        .register(Arc::new(ChainExtension {
            id: ext_id,
            table,
            transient,
        }))
        .expect("register");

    install_extension_schemas(&registry, fx.db.as_ref())
        .await
        .expect("an ALTER on a table an earlier migration created is the extension's own");
    assert_eq!(applied_versions(&fx.pool, ext_id).await, vec![1, 2, 3]);
    assert!(!table_exists(&fx.pool, transient).await);
}
