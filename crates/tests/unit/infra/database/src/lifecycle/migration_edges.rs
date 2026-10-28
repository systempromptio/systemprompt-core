use std::sync::{Arc, Mutex};

use systemprompt_database::{
    DatabaseAdminService, DatabaseProvider, MigrationService, PostgresProvider,
};
use systemprompt_extension::{
    Extension, ExtensionMetadata, LoaderError, Migration, SchemaDefinition,
};
use systemprompt_test_fixtures::install_extension_schemas_with_config;
use tracing_subscriber::layer::SubscriberExt;

use super::installation::{StubExtension, provider_and_db, registry_with, unique_id};
use crate::services::db_helper::test_pool;

struct OwnedExtension {
    id: &'static str,
    schemas: Vec<SchemaDefinition>,
    cross: Vec<&'static str>,
    migrations: Vec<Migration>,
}

impl Extension for OwnedExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "owned",
            version: "0.0.0",
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        self.schemas.clone()
    }

    fn cross_extension_tables(&self) -> Vec<&'static str> {
        self.cross.clone()
    }

    fn migrations(&self) -> Vec<Migration> {
        self.migrations.clone()
    }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for LogBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("log buffer").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogBuffer {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl LogBuffer {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().expect("log buffer").clone()).expect("utf8 logs")
    }

    fn install(&self) -> tracing::subscriber::DefaultGuard {
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(self.clone()),
        );
        tracing::subscriber::set_default(subscriber)
    }
}

async fn provider() -> PostgresProvider {
    PostgresProvider::from_pool(test_pool().await.write_pool())
}

async fn drop(provider: &PostgresProvider, ext_id: &str, table: &str) {
    let _ = provider
        .execute_raw(&format!(
            "DELETE FROM extension_migrations WHERE extension_id = '{ext_id}'"
        ))
        .await;
    let _ = provider
        .execute_raw(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
        .await;
}

#[tokio::test]
async fn an_alter_on_a_foreign_table_is_refused_before_it_runs() {
    let provider = provider().await;
    let ext_id = unique_id("alter_foreign");
    let table = unique_id("alter_foreign_t");
    provider
        .execute_raw(&format!("CREATE TABLE {table} (id BIGINT PRIMARY KEY)"))
        .await
        .expect("foreign table");
    let ext = OwnedExtension {
        id: ext_id,
        schemas: vec![],
        cross: vec![],
        migrations: vec![Migration::new(
            1,
            "reach",
            leak(format!("ALTER TABLE {table} ADD COLUMN v INT;")),
        )],
    };

    let err = MigrationService::new(&provider)
        .run_pending_migrations(&ext)
        .await
        .expect_err("undeclared cross-extension alter");

    match err {
        LoaderError::CrossExtensionAlterUndeclared { table: refused, .. } => {
            assert_eq!(refused, table);
        },
        other => panic!("unexpected error: {other:?}"),
    }
    let columns = provider
        .fetch_all(
            &"SELECT column_name::text AS name FROM information_schema.columns WHERE table_name = $1",
            &[&table],
        )
        .await
        .expect("columns");
    assert_eq!(columns.len(), 1, "the refused alter never ran");
    drop(&provider, ext_id, table).await;
}

#[tokio::test]
async fn alters_on_declared_and_previously_created_tables_run() {
    let provider = provider().await;
    let ext_id = unique_id("alter_owned");
    let declared = unique_id("alter_declared_t");
    let created = unique_id("alter_created_t");
    let owned = unique_id("alter_schema_t");
    provider
        .execute_raw(&format!("CREATE TABLE {declared} (id BIGINT PRIMARY KEY)"))
        .await
        .expect("declared table");
    provider
        .execute_raw(&format!("CREATE TABLE {owned} (id BIGINT PRIMARY KEY)"))
        .await
        .expect("owned table");
    let ext = OwnedExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::new(
            owned,
            format!("CREATE TABLE IF NOT EXISTS {owned} (id BIGINT PRIMARY KEY);"),
        )],
        cross: vec![declared],
        migrations: vec![
            Migration::new(
                1,
                "create",
                leak(format!("CREATE TABLE {created} (id BIGINT PRIMARY KEY);")),
            ),
            Migration::new(
                2,
                "widen",
                leak(format!(
                    "ALTER TABLE {created} ADD COLUMN v INT;\nALTER TABLE {declared} ADD COLUMN v \
                     INT;\nALTER TABLE {owned} ADD COLUMN v INT;"
                )),
            ),
        ],
    };

    let result = MigrationService::new(&provider)
        .run_pending_migrations(&ext)
        .await
        .expect("owned alters run");

    assert_eq!(result.migrations_run, 2);
    for table in [declared, created, owned] {
        let rows = provider
            .fetch_all(
                &"SELECT 1 AS one FROM information_schema.columns WHERE table_name = $1 AND \
                  column_name = 'v'",
                &[&table],
            )
            .await
            .expect("column probe");
        assert_eq!(rows.len(), 1, "{table} gained v");
    }
    drop(&provider, ext_id, declared).await;
    drop(&provider, ext_id, created).await;
    drop(&provider, ext_id, owned).await;
}

async fn table_with_trigger(provider: &PostgresProvider, table: &str) {
    provider
        .execute_batch(&format!(
            "CREATE TABLE {table} (id BIGINT PRIMARY KEY, v INT NOT NULL DEFAULT 0);
             INSERT INTO {table} (id) SELECT g FROM generate_series(1, 20) g;
             ANALYZE {table};
             CREATE OR REPLACE FUNCTION {table}_noop() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN RETURN NEW; END $$;
             CREATE TRIGGER {table}_row AFTER UPDATE ON {table}
                 FOR EACH ROW EXECUTE FUNCTION {table}_noop();"
        ))
        .await
        .expect("trigger table");
}

#[tokio::test(flavor = "current_thread")]
async fn suspended_triggers_are_named_in_the_migration_log() {
    let logs = LogBuffer::default();
    let _guard = logs.install();
    let provider = provider().await;
    let ext_id = unique_id("trg_log");
    let table = unique_id("trg_log_t");
    table_with_trigger(&provider, table).await;
    let ext = StubExtension {
        id: ext_id,
        schemas: vec![],
        seeds: vec![],
        migrations: vec![
            Migration::new(1, "tx", leak(format!("UPDATE {table} SET v = 1;"))),
            Migration::new_no_transaction(
                2,
                "notx",
                leak(format!("-- @no-transaction\nUPDATE {table} SET v = 2;")),
            ),
        ],
    };

    let result = MigrationService::new(&provider)
        .run_pending_migrations(&ext)
        .await
        .expect("migrations run");

    assert_eq!(result.migrations_run, 2);
    let text = logs.text();
    let named = format!("{table}.{table}_row");
    assert_eq!(
        text.matches("Row triggers suspended for migration").count(),
        2,
        "{text}"
    );
    assert!(text.contains(&named), "{text}");
    assert!(text.contains("Migration applied"), "{text}");
    drop(&provider, ext_id, table).await;
}

#[tokio::test]
async fn a_costed_migration_over_a_populated_table_applies() {
    let provider = provider().await;
    let ext_id = unique_id("costed");
    let table = unique_id("costed_t");
    table_with_trigger(&provider, table).await;
    let ext = StubExtension {
        id: ext_id,
        schemas: vec![],
        seeds: vec![],
        migrations: vec![Migration::new(
            1,
            "backfill",
            leak(format!(
                "-- @cost: rows=20 measured=0.1s triggers=suspended\nUPDATE {table} SET v = 3;"
            )),
        )],
    };

    let result = MigrationService::new(&provider)
        .run_pending_migrations(&ext)
        .await
        .expect("costed migration");

    assert_eq!(result.migrations_run, 1);
    let rows = provider
        .fetch_all(
            &format!("SELECT count(*)::bigint AS n FROM {table} WHERE v = 3"),
            &[],
        )
        .await
        .expect("count");
    assert_eq!(rows[0]["n"].as_i64(), Some(20));
    drop(&provider, ext_id, table).await;
}

#[tokio::test]
async fn counted_table_listing_reports_live_row_counts() {
    let provider = provider().await;
    let table = unique_id("counted_t");
    provider
        .execute_batch(&format!(
            "CREATE TABLE {table} (id BIGINT PRIMARY KEY);
             INSERT INTO {table} (id) VALUES (1), (2), (3);"
        ))
        .await
        .expect("counted table");
    let pool = test_pool().await;
    let service = DatabaseAdminService::new(pool.write_pool());

    let tables = service
        .list_tables_counted()
        .await
        .expect("counted listing");

    let found = tables
        .iter()
        .find(|t| t.name == table)
        .expect("listed table");
    assert_eq!(found.row_count, 3);
    let _ = provider
        .execute_raw(&format!("DROP TABLE IF EXISTS {table}"))
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn an_unmeasured_pending_rewrite_of_a_populated_hot_table_is_warned() {
    let logs = LogBuffer::default();
    let _guard = logs.install();
    let (provider, db) = provider_and_db().await;
    provider
        .execute_batch(
            "INSERT INTO logs (level, module, message) VALUES ('INFO', 'cost_warning_test', 'seed');
             ANALYZE logs;",
        )
        .await
        .expect("populate logs");
    let ext_id = unique_id("hot_rewrite");
    let table = unique_id("hot_rewrite_t");
    let ext = StubExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::new(
            table,
            format!("CREATE TABLE IF NOT EXISTS {table} (id BIGINT PRIMARY KEY);"),
        )],
        seeds: vec![],
        migrations: vec![Migration::new(
            1,
            "rewrite_logs",
            "UPDATE logs SET level = level WHERE module = 'cost_warning_test';",
        )],
    };

    let outcome = install_extension_schemas_with_config(&registry_with(ext), &provider, &[]).await;

    let text = logs.text();
    assert!(
        text.contains("Pending migration rewrites a populated hot table"),
        "outcome {outcome:?}\n{text}"
    );
    assert!(text.contains("UPDATE on logs"), "{text}");
    let _ = provider
        .execute_raw("DELETE FROM logs WHERE module = 'cost_warning_test'")
        .await;
    let _ = provider
        .execute_raw(&format!(
            "DELETE FROM extension_migrations WHERE extension_id = '{ext_id}'"
        ))
        .await;
    super::installation::drop_table(&db, table).await;
}

#[tokio::test]
async fn a_tombstone_cannot_be_marked_applied() {
    let provider = provider().await;
    let ext_id = unique_id("tomb_mark");
    let ext = StubExtension {
        id: ext_id,
        schemas: vec![],
        seeds: vec![],
        migrations: vec![Migration::tombstone(1, "spent")],
    };

    let err = MigrationService::new(&provider)
        .mark_applied(&ext, 1)
        .await
        .expect_err("tombstone refused");

    match err {
        LoaderError::MigrationFailed { message, .. } => {
            assert!(message.contains("is a tombstone"), "{message}");
        },
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_failing_declarative_routine_aborts_the_install_with_its_statement() {
    let (provider, db) = provider_and_db().await;
    let ext_id = unique_id("routine_fail");
    let table = unique_id("routine_fail_t");
    let ext = StubExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::new(
            table,
            format!(
                "CREATE TABLE IF NOT EXISTS {table} (id BIGINT PRIMARY KEY);\n\
                 CREATE OR REPLACE FUNCTION {table}_fn() RETURNS no_such_type_{table} \
                 LANGUAGE sql AS $$ SELECT NULL $$;"
            ),
        )],
        seeds: vec![],
        migrations: vec![],
    };

    let err = install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("routine with an unknown return type");

    let rendered = format!("{err:?}");
    assert!(rendered.contains("routine pre-pass"), "{rendered}");
    assert!(rendered.contains("Statement 1/1 failed"), "{rendered}");
    super::installation::drop_table(&db, table).await;
}

#[tokio::test]
async fn a_failing_schema_statement_rolls_back_and_names_the_statement() {
    let (provider, db) = provider_and_db().await;
    let ext_id = unique_id("phase_fail");
    let table = unique_id("phase_fail_t");
    let ext = StubExtension {
        id: ext_id,
        schemas: vec![SchemaDefinition::new(
            table,
            format!(
                "CREATE TABLE IF NOT EXISTS {table} (id BIGINT PRIMARY KEY);\n\
                 CREATE INDEX IF NOT EXISTS {table}_missing ON {table} (no_such_column);"
            ),
        )],
        seeds: vec![],
        migrations: vec![],
    };

    let err = install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("an index over a missing column fails the phase");

    let rendered = format!("{err:?}");
    assert!(rendered.contains("no_such_column"), "{rendered}");
    assert!(
        !super::installation::table_exists(&db, table).await,
        "the failed phase rolled its table back"
    );
}
