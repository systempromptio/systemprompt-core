//! DB tests for the migration runner's failure and ledger paths: a failing
//! `@no-transaction` migration restores the triggers it suspended and is not
//! recorded, and a `@compat: breaking` migration is written to the
//! compatibility ledger before it runs.

use crate::services::db_helper::test_pool;
use systemprompt_database::{DatabaseProvider, MigrationService, PostgresProvider};
use systemprompt_extension::{Extension, ExtensionMetadata, Migration};
use systemprompt_identifiers::ExtensionId;

struct StubExtension {
    id: &'static str,
    migrations: Vec<Migration>,
}

impl Extension for StubExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "stub",
            version: "0.0.0",
        }
    }

    fn migrations(&self) -> Vec<Migration> {
        self.migrations.clone()
    }
}

async fn provider() -> PostgresProvider {
    PostgresProvider::from_pool(test_pool().await.write_pool())
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn unique(prefix: &str) -> String {
    format!(
        "{prefix}_{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    )
}

async fn reset(provider: &PostgresProvider, extension: &str, table: &str) {
    let _ = provider
        .execute_raw(&format!(
            "DELETE FROM extension_migrations WHERE extension_id = '{extension}'"
        ))
        .await;
    provider
        .execute_batch(&format!(
            "DROP TABLE IF EXISTS {table} CASCADE;
             CREATE TABLE {table} (id BIGINT PRIMARY KEY, v INT NOT NULL DEFAULT 0);
             INSERT INTO {table} (id) SELECT g FROM generate_series(1, 5) g;
             CREATE OR REPLACE FUNCTION {table}_noop() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN RETURN NEW; END $$;
             CREATE TRIGGER {table}_row AFTER UPDATE ON {table}
                 FOR EACH ROW EXECUTE FUNCTION {table}_noop();"
        ))
        .await
        .expect("fixture table");
}

async fn trigger_enabled(provider: &PostgresProvider, table: &str) -> String {
    provider
        .fetch_all(
            &"SELECT tgenabled::text AS state FROM pg_trigger WHERE tgrelid = to_regclass($1) \
              AND NOT tgisinternal",
            &[&table],
        )
        .await
        .expect("trigger state")
        .first()
        .and_then(|row| row["state"].as_str().map(str::to_owned))
        .expect("one trigger")
}

#[tokio::test]
async fn a_failing_no_transaction_migration_restores_triggers_and_is_not_recorded() {
    let provider = provider().await;
    let ext_id = leak(unique("notx_fail"));
    let table = leak(unique("notx_fail_t"));
    reset(&provider, ext_id, table).await;
    let ext = StubExtension {
        id: ext_id,
        migrations: vec![Migration::new_no_transaction(
            1,
            "broken",
            leak(format!(
                "-- @no-transaction\nUPDATE {table} SET v = 1;\nUPDATE {table} SET missing_column = 1;"
            )),
        )],
    };

    let err = MigrationService::new(&provider)
        .run_pending_migrations(&ext)
        .await
        .expect_err("the second statement fails");

    assert!(
        format!("{err:?}").contains("Failed to execute migration 1 (broken)"),
        "{err:?}"
    );
    assert_eq!(
        trigger_enabled(&provider, table).await,
        "O",
        "trigger restored after failure"
    );
    let applied = MigrationService::new(&provider)
        .get_applied_migrations(&ExtensionId::new(ext_id))
        .await
        .expect("applied");
    assert!(applied.is_empty(), "a failed migration is never recorded");
    let _ = provider
        .execute_raw(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
        .await;
}

#[tokio::test]
async fn a_successful_no_transaction_migration_is_recorded_once() {
    let provider = provider().await;
    let ext_id = leak(unique("notx_ok"));
    let table = leak(unique("notx_ok_t"));
    reset(&provider, ext_id, table).await;
    let ext = StubExtension {
        id: ext_id,
        migrations: vec![Migration::new_no_transaction(
            1,
            "index",
            leak(format!(
                "-- @no-transaction\nCREATE INDEX CONCURRENTLY IF NOT EXISTS {table}_v ON {table} (v);"
            )),
        )],
    };
    let svc = MigrationService::new(&provider);

    let first = svc.run_pending_migrations(&ext).await.expect("apply");
    let second = svc.run_pending_migrations(&ext).await.expect("re-run");

    assert_eq!(first.migrations_run, 1);
    assert_eq!(second.migrations_run, 0);
    let applied = svc
        .get_applied_migrations(&ExtensionId::new(ext_id))
        .await
        .expect("applied");
    assert_eq!(applied.len(), 1);
    let _ = provider
        .execute_raw(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
        .await;
}

#[tokio::test]
async fn a_breaking_migration_is_written_to_the_compat_ledger() {
    let provider = provider().await;
    let ext_id = leak(unique("compat_brk"));
    let table = leak(unique("compat_brk_t"));
    reset(&provider, ext_id, table).await;
    let ext = StubExtension {
        id: ext_id,
        migrations: vec![
            Migration::new(1, "additive", leak(format!("UPDATE {table} SET v = 1;"))),
            Migration::new(
                2,
                "contract",
                leak(format!("-- @compat: breaking\nUPDATE {table} SET v = 2;")),
            ),
        ],
    };
    let svc = MigrationService::new(&provider);
    let id = ExtensionId::new(ext_id);

    svc.run_pending_migrations(&ext).await.expect("apply");

    assert_eq!(svc.breaking_versions(&id).await.expect("ledger"), vec![2]);
    assert!(
        svc.breaking_versions(&ExtensionId::new(leak(unique("compat_none"))))
            .await
            .expect("ledger")
            .is_empty()
    );
    let _ = provider
        .execute_raw(&format!(
            "DELETE FROM extension_migration_compat WHERE extension_id = '{ext_id}'"
        ))
        .await;
    let _ = provider
        .execute_raw(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
        .await;
}
