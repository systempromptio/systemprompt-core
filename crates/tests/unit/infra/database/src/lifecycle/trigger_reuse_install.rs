//! A full install leaves a declarative trigger in place when its statements
//! and its live definition are unchanged since it was last applied, so the
//! table it fires on is never locked; a trigger changed behind the
//! installer's back, or a forced full install, re-creates it. A trigger's
//! `pg_trigger` row version (`xmin`) changes whenever it is dropped and
//! created or replaced, so an unchanged version means it was left alone.

use systemprompt_database::{DbPool, MigrationConfig, install_extension_schemas_full};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_test_fixtures::DisposableDb;

async fn install(pool: &DbPool, config: MigrationConfig) {
    install_extension_schemas_full(
        &ExtensionRegistry::discover().expect("registry"),
        pool.write(),
        &[],
        config,
    )
    .await
    .expect("install");
}

async fn stale_fingerprint(pool: &sqlx::PgPool) {
    sqlx::query("UPDATE schema_install_fingerprint SET fingerprint = 'stale'")
        .execute(pool)
        .await
        .expect("stale fingerprint");
}

async fn recorded_trigger_versions(pool: &sqlx::PgPool) -> Vec<(String, String, String)> {
    sqlx::query_as(
        "SELECT d.relation, d.trigger_name, t.xmin::text FROM schema_trigger_digests d \
         JOIN pg_trigger t ON t.tgrelid = to_regclass(d.relation) AND t.tgname = d.trigger_name \
         ORDER BY 1, 2",
    )
    .fetch_all(pool)
    .await
    .expect("recorded triggers")
}

async fn trigger_version(pool: &sqlx::PgPool, relation: &str, name: &str) -> Option<String> {
    sqlx::query_scalar(
        "SELECT t.xmin::text FROM pg_trigger t \
         WHERE t.tgrelid = to_regclass($1) AND t.tgname = $2",
    )
    .bind(relation)
    .bind(name)
    .fetch_optional(pool)
    .await
    .expect("trigger version")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_full_install_leaves_unchanged_triggers_in_place() {
    let database = DisposableDb::with_schema("trigger_reuse_unchanged").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    let before = recorded_trigger_versions(&raw).await;
    assert!(
        !before.is_empty(),
        "a complete install records a digest for every declarative trigger"
    );

    stale_fingerprint(&raw).await;
    install(&pool, MigrationConfig::default()).await;

    assert_eq!(
        recorded_trigger_versions(&raw).await,
        before,
        "an unchanged trigger is not dropped and re-created by a full install"
    );

    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_trigger_changed_out_of_band_is_reapplied() {
    let database = DisposableDb::with_schema("trigger_reuse_drift").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    let (relation, name, definition): (String, String, String) = sqlx::query_as(
        "SELECT d.relation, d.trigger_name, pg_get_triggerdef(t.oid) FROM schema_trigger_digests d \
         JOIN pg_trigger t ON t.tgrelid = to_regclass(d.relation) AND t.tgname = d.trigger_name \
         WHERE pg_get_triggerdef(t.oid) LIKE '%FOR EACH ROW%' \
           AND pg_get_triggerdef(t.oid) NOT LIKE '% WHEN %' \
           AND pg_get_triggerdef(t.oid) NOT LIKE '%REFERENCING%' \
         ORDER BY 1, 2 LIMIT 1",
    )
    .fetch_one(raw.as_ref())
    .await
    .expect("a row-level trigger with a digest");
    let drifted = definition
        .replacen("CREATE TRIGGER", "CREATE OR REPLACE TRIGGER", 1)
        .replacen("FOR EACH ROW", "FOR EACH STATEMENT", 1);
    sqlx::query(sqlx::AssertSqlSafe(drifted))
        .execute(raw.as_ref())
        .await
        .expect("change the trigger out of band");

    stale_fingerprint(&raw).await;
    install(&pool, MigrationConfig::default()).await;

    let restored: String = sqlx::query_scalar(
        "SELECT pg_get_triggerdef(t.oid) FROM pg_trigger t \
         WHERE t.tgrelid = to_regclass($1) AND t.tgname = $2",
    )
    .bind(&relation)
    .bind(&name)
    .fetch_one(raw.as_ref())
    .await
    .expect("trigger definition");
    assert_eq!(
        restored, definition,
        "the declared definition of {name} on {relation} is re-applied"
    );

    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_forced_full_install_recreates_every_trigger() {
    let database = DisposableDb::with_schema("trigger_reuse_forced").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    let before = recorded_trigger_versions(&raw).await;
    let (relation, name, version) = before.first().cloned().expect("a recorded trigger");

    install(
        &pool,
        MigrationConfig {
            force_full_install: true,
            ..MigrationConfig::default()
        },
    )
    .await;

    let after = trigger_version(&raw, &relation, &name)
        .await
        .expect("the trigger still exists");
    assert_ne!(after, version, "a forced full install re-creates {name}");

    drop(pool);
    database.drop_now().await;
}
