//! A database a newer release migrated is ahead of an older binary, not
//! behind it: the older binary keeps booting, applies nothing, and leaves the
//! newer release's checksums alone.
use super::*;
use systemprompt_database::schema_currency;

fn newer(f: &Fixture) -> SlotExt {
    SlotExt {
        id: f.ext_id,
        table: f.table,
        schema_sql: f.schema_sql,
        migrations: vec![
            Migration::new(7, "original", f.create_sql),
            Migration::new(8, "expand", f.reuse_sql),
        ],
    }
}

fn older(f: &Fixture, sql: &'static str) -> SlotExt {
    SlotExt {
        id: f.ext_id,
        table: f.table,
        schema_sql: f.schema_sql,
        migrations: vec![Migration::new(7, "original", sql)],
    }
}

async fn migrate_as_newer(f: &Fixture) {
    MigrationService::new(f.db.write())
        .run_pending_migrations(&newer(f))
        .await
        .expect("newer release migrates");
    assert_eq!(applied_versions(&f.pool, f.ext_id).await, vec![7, 8]);
}

fn registry_of(ext: SlotExt) -> ExtensionRegistry {
    let mut registry = ExtensionRegistry::new();
    registry.register(Arc::new(ext)).expect("register");
    registry
}

async fn stored_checksum(f: &Fixture, version: i32) -> String {
    query("SELECT checksum FROM extension_migrations WHERE extension_id=$1 AND version=$2")
        .bind(f.ext_id)
        .bind(version)
        .fetch_one(&f.pool)
        .await
        .expect("ledger row")
        .get("checksum")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_older_binary_reports_the_newer_migration_as_ahead_and_stays_current() {
    let f = fixture().await;
    migrate_as_newer(&f).await;

    let registry = registry_of(older(&f, f.create_sql));
    let currency = schema_currency(f.db.write(), &registry, &[])
        .await
        .expect("currency check");

    assert!(currency.is_current(), "an ahead schema is serviceable");
    assert!(currency.is_ahead());
    assert_eq!(currency.ahead.len(), 1);
    assert_eq!(currency.ahead[0].version, 8);
    assert_eq!(currency.ahead[0].name, "expand");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn checksum_drift_in_an_ahead_extension_is_left_to_the_newer_release() {
    let f = fixture().await;
    migrate_as_newer(&f).await;
    let before = stored_checksum(&f, 7).await;

    let drifted = older(&f, "SELECT 7;");
    let registry = registry_of(older(&f, "SELECT 7;"));
    let currency = schema_currency(f.db.write(), &registry, &[])
        .await
        .expect("currency check");
    assert!(currency.drift.is_empty());
    assert_eq!(currency.superseded_drift.len(), 1);
    assert!(currency.is_current());

    let result = MigrationService::new(f.db.write())
        .run_pending_migrations(&drifted)
        .await
        .expect("an older binary must not fail on a schema a newer one owns");
    assert_eq!((result.migrations_run, result.migrations_skipped), (0, 0));
    assert_eq!(
        stored_checksum(&f, 7).await,
        before,
        "the older binary must not rewrite the newer release's ledger"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_older_install_against_an_ahead_database_applies_nothing_and_reports_it() {
    let f = fixture().await;
    migrate_as_newer(&f).await;

    let registry = registry_of(older(&f, "SELECT 7;"));
    let report = install_extension_schemas(&registry, f.db.as_ref())
        .await
        .expect("install defers to the newer release");

    assert!(report.is_ahead());
    assert_eq!(report.ahead[0].version, 8);
    assert!(column_exists(&f.pool, f.table, "refilled_slot").await);
    assert_eq!(applied_versions(&f.pool, f.ext_id).await, vec![7, 8]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn drift_without_a_newer_migration_still_fails_currency() {
    let f = fixture().await;
    apply_original(&f).await;

    let registry = registry_of(older(&f, "SELECT 7;"));
    let currency = schema_currency(f.db.write(), &registry, &[])
        .await
        .expect("currency check");

    assert!(!currency.is_ahead());
    assert_eq!(currency.drift.len(), 1);
    assert!(!currency.is_current());
}
