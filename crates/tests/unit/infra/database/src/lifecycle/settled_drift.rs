use systemprompt_database::{
    DbPool, MigrationConfig, install_extension_schemas_full, schema_catalog_digest,
};
use systemprompt_extension::{ExtensionRegistry, LoaderError};
use systemprompt_test_fixtures::DisposableDb;

async fn install_with(pool: &DbPool, config: MigrationConfig) -> Result<(), LoaderError> {
    install_extension_schemas_full(
        &ExtensionRegistry::discover().expect("registry"),
        pool.write(),
        &[],
        config,
    )
    .await
    .map(|_| ())
}

async fn install(pool: &DbPool) -> Result<(), LoaderError> {
    install_with(pool, MigrationConfig::default()).await
}

async fn stamp(pool: &sqlx::PgPool) -> (String, chrono::DateTime<chrono::Utc>) {
    sqlx::query_as("SELECT fingerprint, recorded_at FROM schema_install_fingerprint")
        .fetch_one(pool)
        .await
        .expect("stamp")
}

async fn scalar(pool: &sqlx::PgPool, sql: &str) -> String {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_owned()))
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
}

async fn exec(pool: &sqlx::PgPool, sql: &str) {
    sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_replaced_view_is_restored_by_the_next_install() {
    let database = DisposableDb::with_schema("settled_view_drift").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    let view = scalar(
        &raw,
        "SELECT c.oid::regclass::text FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE c.relkind = 'v' AND n.nspname = 'public' ORDER BY 1 LIMIT 1",
    )
    .await;
    let defn = |v: &str| format!("SELECT pg_get_viewdef('{v}'::regclass)");
    let original = scalar(&raw, &defn(&view)).await;
    exec(
        &raw,
        &format!(
            "CREATE OR REPLACE VIEW {view} AS SELECT * FROM ({}) AS older WHERE false",
            original.trim_end_matches(';')
        ),
    )
    .await;
    assert_ne!(scalar(&raw, &defn(&view)).await, original);

    install(&pool).await.expect("install");

    assert_eq!(
        scalar(&raw, &defn(&view)).await,
        original,
        "{view} restored"
    );
    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_replaced_function_is_restored_by_the_next_install() {
    let database = DisposableDb::with_schema("settled_function_drift").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    let defn = "SELECT pg_get_functiondef('sp_stamp_change()'::regprocedure)";
    let original = scalar(&raw, defn).await;
    exec(
        &raw,
        "CREATE OR REPLACE FUNCTION sp_stamp_change() RETURNS trigger LANGUAGE plpgsql \
         AS $$ BEGIN RETURN NEW; END; $$",
    )
    .await;
    assert_ne!(scalar(&raw, defn).await, original);

    install(&pool).await.expect("install");

    assert_eq!(scalar(&raw, defn).await, original);
    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_dropped_trigger_is_restored_by_the_next_install() {
    let database = DisposableDb::with_schema("settled_trigger_drift").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    let target = scalar(
        &raw,
        "SELECT t.tgrelid::regclass::text || '|' || t.tgname FROM pg_trigger t \
         WHERE NOT t.tgisinternal ORDER BY 1 LIMIT 1",
    )
    .await;
    let (table, trigger) = target.split_once('|').expect("table|trigger");
    exec(&raw, &format!("DROP TRIGGER \"{trigger}\" ON {table}")).await;

    install(&pool).await.expect("install");

    let present = scalar(
        &raw,
        &format!(
            "SELECT count(*)::text FROM pg_trigger WHERE tgname = '{trigger}' \
             AND tgrelid = '{table}'::regclass"
        ),
    )
    .await;
    assert_eq!(present, "1", "{trigger} on {table} restored");
    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_forced_install_runs_the_full_path_on_a_settled_database() {
    let database = DisposableDb::with_schema("settled_forced").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    let (_, before) = stamp(&raw).await;

    install(&pool).await.expect("settled install");
    assert_eq!(
        stamp(&raw).await.1,
        before,
        "a settled install records nothing"
    );

    install_with(
        &pool,
        MigrationConfig {
            force_full_install: true,
            ..MigrationConfig::default()
        },
    )
    .await
    .expect("forced install");
    assert!(
        stamp(&raw).await.1 > before,
        "the forced install ran the full path and re-recorded"
    );
    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_settled_path_still_validates_required_columns() {
    let database = DisposableDb::with_schema("settled_validates").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    exec(&raw, "ALTER TABLE release_stamps DROP COLUMN version").await;
    let (stored, _) = stamp(&raw).await;
    let (binary, _) = stored.split_once(':').expect("stamp shape");
    let digest = schema_catalog_digest(pool.write()).await.expect("digest");
    sqlx::query("UPDATE schema_install_fingerprint SET fingerprint = $1")
        .bind(format!("{binary}:{digest}"))
        .execute(raw.as_ref())
        .await
        .expect("restamp as settled");

    let error = install(&pool)
        .await
        .expect_err("a missing required column fails the settled install");
    assert!(
        error.to_string().contains("version") || format!("{error:?}").contains("version"),
        "{error:?}"
    );
    let (after, _) = stamp(&raw).await;
    assert_eq!(
        after,
        format!("{binary}:{digest}"),
        "the settled path ran (no re-record)"
    );
    drop(pool);
    database.drop_now().await;
}
