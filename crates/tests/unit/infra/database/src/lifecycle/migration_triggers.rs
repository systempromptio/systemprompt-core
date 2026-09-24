//! DB tests for the three multi-release upgrade guarantees: a migration does
//! not fire the row triggers on the tables it writes (unless it declares
//! `triggers=live`), and restores exactly the ones it suspended; a boot
//! refuses a trigger whose routine writes a relation that is gone.

use crate::services::db_helper::pool_or_skip;
use systemprompt_database::{
    DatabaseProvider, MigrationService, PostgresProvider, check_trigger_routines,
};
use systemprompt_extension::{Extension, ExtensionMetadata, LoaderError, Migration};

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

async fn provider_or_skip() -> Option<PostgresProvider> {
    let db = pool_or_skip().await?;
    let pg = db.write_pool_arc().ok()?;
    Some(PostgresProvider::from_pool(pg))
}

// Why: the "foreign" trigger stands in for another extension's stale one —
// it fails every row it sees, so any firing fails the migration.
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
             INSERT INTO {table} (id) SELECT g FROM generate_series(1, 50) g;
             CREATE OR REPLACE FUNCTION {table}_refuse() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN RAISE EXCEPTION 'stale trigger fired'; END $$;
             CREATE TRIGGER {table}_foreign AFTER UPDATE ON {table}
                 FOR EACH ROW EXECUTE FUNCTION {table}_refuse();
             CREATE TRIGGER {table}_disabled AFTER UPDATE ON {table}
                 FOR EACH ROW EXECUTE FUNCTION {table}_refuse();
             ALTER TABLE {table} DISABLE TRIGGER {table}_disabled;"
        ))
        .await
        .expect("fixture table");
}

async fn trigger_state(provider: &PostgresProvider, table: &str) -> Vec<(String, String)> {
    provider
        .fetch_all(
            &"SELECT tgname::text AS name, tgenabled::text AS state FROM pg_trigger WHERE \
              tgrelid = to_regclass($1) AND NOT tgisinternal ORDER BY tgname",
            &[&table],
        )
        .await
        .expect("trigger state")
        .into_iter()
        .map(|row| {
            (
                row["name"].as_str().unwrap_or_default().to_owned(),
                row["state"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

#[tokio::test]
async fn a_backfill_does_not_fire_row_triggers_and_restores_them() {
    let Some(provider) = provider_or_skip().await else {
        return;
    };
    reset(&provider, "trg_suspend", "trg_suspend_t").await;
    let ext = StubExtension {
        id: "trg_suspend",
        migrations: vec![Migration::new(
            1,
            "backfill",
            "UPDATE trg_suspend_t SET v = 1;",
        )],
    };
    MigrationService::new(&provider)
        .run_pending_migrations(&ext)
        .await
        .expect("the stale trigger must not fire during the backfill");

    let state = trigger_state(&provider, "trg_suspend_t").await;
    assert_eq!(
        state,
        vec![
            ("trg_suspend_t_disabled".to_owned(), "D".to_owned()),
            ("trg_suspend_t_foreign".to_owned(), "O".to_owned()),
        ],
        "the suspended trigger is re-enabled; the one already disabled stays disabled"
    );
}

#[tokio::test]
async fn a_no_transaction_backfill_suspends_and_restores_too() {
    let Some(provider) = provider_or_skip().await else {
        return;
    };
    reset(&provider, "trg_notx", "trg_notx_t").await;
    let ext = StubExtension {
        id: "trg_notx",
        migrations: vec![Migration::new_no_transaction(
            1,
            "backfill",
            "-- @no-transaction\nUPDATE trg_notx_t SET v = 1;",
        )],
    };
    MigrationService::new(&provider)
        .run_pending_migrations(&ext)
        .await
        .expect("the stale trigger must not fire during the backfill");
    let state = trigger_state(&provider, "trg_notx_t").await;
    assert!(state.contains(&("trg_notx_t_foreign".to_owned(), "O".to_owned())));
}

#[tokio::test]
async fn a_migration_declaring_live_triggers_fires_them() {
    let Some(provider) = provider_or_skip().await else {
        return;
    };
    reset(&provider, "trg_live", "trg_live_t").await;
    let ext = StubExtension {
        id: "trg_live",
        migrations: vec![Migration::new(
            1,
            "backfill",
            "-- @cost: rows=50 measured=10ms triggers=live\nUPDATE trg_live_t SET v = 1;",
        )],
    };
    let err = MigrationService::new(&provider)
        .run_pending_migrations(&ext)
        .await
        .expect_err("triggers=live keeps the trigger on");
    assert!(err.to_string().contains("stale trigger fired"), "{err}");
}

#[tokio::test]
async fn a_trigger_writing_a_dropped_relation_refuses_the_boot() {
    let Some(provider) = provider_or_skip().await else {
        return;
    };
    provider
        .execute_batch(
            "DROP TABLE IF EXISTS trg_dangling_t CASCADE;
             DROP TABLE IF EXISTS trg_dangling_target;
             CREATE TABLE trg_dangling_target (id BIGINT);
             CREATE TABLE trg_dangling_t (id BIGINT PRIMARY KEY);
             CREATE OR REPLACE FUNCTION trg_dangling_write() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN INSERT INTO trg_dangling_target (id) VALUES (NEW.id); RETURN NEW; END $$;
             CREATE TRIGGER trg_dangling AFTER INSERT ON trg_dangling_t
                 FOR EACH ROW EXECUTE FUNCTION trg_dangling_write();",
        )
        .await
        .expect("fixture");

    check_trigger_routines(&provider)
        .await
        .expect("a live target is not dangling");

    provider
        .execute_raw("DROP TABLE trg_dangling_target")
        .await
        .expect("drop target");
    let err = check_trigger_routines(&provider)
        .await
        .expect_err("a trigger writing a dropped table must refuse the boot");
    match err {
        LoaderError::DanglingTriggerRoutine {
            trigger, relation, ..
        } => {
            assert_eq!(trigger, "trg_dangling");
            assert_eq!(relation, "trg_dangling_target");
        },
        other => panic!("expected DanglingTriggerRoutine, got {other:?}"),
    }

    provider
        .execute_batch("DROP TABLE trg_dangling_t CASCADE; DROP FUNCTION trg_dangling_write()")
        .await
        .expect("cleanup");
}
