//! The schema install is release-ordered: a binary older than the release
//! recorded under the `schema_install` stamp applies nothing — a routine body,
//! a view or a trigger a newer release wrote is left exactly as it wrote it —
//! and refuses when the schema it reads is not present.

use systemprompt_database::{DbPool, MigrationConfig, install_admitted_schemas};
use systemprompt_extension::LoaderError;
use systemprompt_test_fixtures::{DisposableDb, schema_admitted};

const NEWER: &str = "9999.0.0";

async fn install(pool: &DbPool) -> Result<(), LoaderError> {
    install_admitted_schemas(
        schema_admitted().expect("core members").built(),
        pool.write(),
        MigrationConfig::default(),
    )
    .await
    .map(|_| ())
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

async fn recorded(pool: &sqlx::PgPool) -> Option<String> {
    sqlx::query_scalar("SELECT version FROM release_stamps WHERE name = 'schema_install'")
        .fetch_optional(pool)
        .await
        .expect("release stamp")
}

async fn newer_release_installed(pool: &sqlx::PgPool) {
    exec(
        pool,
        &format!(
            "INSERT INTO release_stamps (name, version) VALUES ('schema_install', '{NEWER}') \
             ON CONFLICT (name) DO UPDATE SET version = EXCLUDED.version"
        ),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_install_records_its_release() {
    let database = DisposableDb::with_schema("release_order_record").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();

    install(&pool).await.expect("install");

    let release = recorded(&raw)
        .await
        .expect("the install recorded its release");
    assert!(!release.is_empty());
    assert_ne!(release, NEWER);
    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_older_release_leaves_a_newer_routine_view_and_trigger_unchanged() {
    let database = DisposableDb::with_schema("release_order_objects").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    install(&pool).await.expect("release N installs");

    exec(
        &raw,
        "CREATE OR REPLACE FUNCTION sp_stamp_change() RETURNS trigger LANGUAGE plpgsql \
         AS $$ BEGIN NEW.updated_at := now(); RETURN NEW; END; $$",
    )
    .await;
    let function = "SELECT pg_get_functiondef('sp_stamp_change()'::regprocedure)";
    let newer_function = scalar(&raw, function).await;

    let view = scalar(
        &raw,
        "SELECT c.oid::regclass::text FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE c.relkind = 'v' AND n.nspname = 'public' ORDER BY 1 LIMIT 1",
    )
    .await;
    let view_def = format!("SELECT pg_get_viewdef('{view}'::regclass)");
    let original_view = scalar(&raw, &view_def).await;
    exec(
        &raw,
        &format!(
            "CREATE OR REPLACE VIEW {view} AS SELECT * FROM ({}) AS newer WHERE true",
            original_view.trim_end_matches(';')
        ),
    )
    .await;
    let newer_view = scalar(&raw, &view_def).await;

    let target = scalar(
        &raw,
        "SELECT t.tgrelid::regclass::text || '|' || t.tgname FROM pg_trigger t \
         WHERE NOT t.tgisinternal ORDER BY 1 LIMIT 1",
    )
    .await;
    let (table, trigger) = target.split_once('|').expect("table|trigger");
    exec(&raw, &format!("DROP TRIGGER \"{trigger}\" ON {table}")).await;
    newer_release_installed(&raw).await;

    install(&pool)
        .await
        .expect("release N boots read-only on the N+1 schema it can read");

    assert_eq!(
        scalar(&raw, function).await,
        newer_function,
        "the N+1 routine body is unchanged"
    );
    assert_eq!(
        scalar(&raw, &view_def).await,
        newer_view,
        "the N+1 view is unchanged"
    );
    let present = scalar(
        &raw,
        &format!(
            "SELECT count(*)::text FROM pg_trigger WHERE tgname = '{trigger}' \
             AND tgrelid = '{table}'::regclass"
        ),
    )
    .await;
    assert_eq!(present, "0", "the trigger N+1 dropped stays dropped");
    assert_eq!(recorded(&raw).await.as_deref(), Some(NEWER));
    drop(pool);
    database.drop_now().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_older_release_refuses_when_the_schema_it_reads_is_absent() {
    let database = DisposableDb::with_schema("release_order_refusal").await;
    let pool = database.test_pool().await;
    let raw = pool.write_pool();
    install(&pool).await.expect("release N installs");
    let ledger = "SELECT count(*)::text FROM extension_migrations WHERE extension_id = 'database'";
    exec(
        &raw,
        "DELETE FROM extension_migrations WHERE extension_id = 'database' AND version = \
         (SELECT max(version) FROM extension_migrations WHERE extension_id = 'database')",
    )
    .await;
    let rows = scalar(&raw, ledger).await;
    newer_release_installed(&raw).await;

    let error = install(&pool)
        .await
        .expect_err("release N refuses a schema with a migration it has not run");

    assert!(
        error.to_string().contains("Schema install refused"),
        "typed refusal, got: {error}"
    );
    assert_eq!(
        scalar(&raw, ledger).await,
        rows,
        "the refused install ran nothing"
    );
    drop(pool);
    database.drop_now().await;
}
