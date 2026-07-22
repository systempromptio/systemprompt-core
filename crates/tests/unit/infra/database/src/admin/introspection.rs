//! DB-backed tests for `DatabaseAdminService`.
//!
//! `list_tables` / `get_database_info` scan the whole `public` schema, which
//! races with sibling tests that create and drop temp tables, so those two
//! run against an isolated throwaway database. The per-table operations use
//! uniquely-named temp tables in the shared database.

use std::sync::Arc;

use systemprompt_database::{Database, DatabaseAdminService, RepositoryError, SafeIdentifier};
use systemprompt_test_fixtures::fixture_database_url;

use crate::services::db_helper::pool;

fn unique_table() -> String {
    format!("admin_introspect_{}", uuid::Uuid::new_v4().simple())
}

async fn write_pool() -> Option<Arc<sqlx::PgPool>> {
    let db = pool().await?;
    db.write_pool_arc().ok()
}

async fn create_fixture_table(pool: &sqlx::PgPool, table: &str) {
    let ddl = format!(
        "CREATE TABLE \"{table}\" (id BIGINT PRIMARY KEY, label TEXT NOT NULL DEFAULT 'x', note \
         TEXT)"
    );
    sqlx::query(sqlx::AssertSqlSafe(ddl))
        .execute(pool)
        .await
        .expect("create fixture table");
}

async fn drop_fixture_table(pool: &sqlx::PgPool, table: &str) {
    let ddl = format!("DROP TABLE IF EXISTS \"{table}\"");
    let _ = sqlx::query(sqlx::AssertSqlSafe(ddl)).execute(pool).await;
}

#[tokio::test]
async fn describe_table_reports_columns_pk_nullability_and_row_count() {
    let Some(pg) = write_pool().await else { return };
    let table = unique_table();
    create_fixture_table(&pg, &table).await;
    let insert = format!("INSERT INTO \"{table}\" (id, note) VALUES (1, NULL), (2, 'b')");
    sqlx::query(sqlx::AssertSqlSafe(insert))
        .execute(&*pg)
        .await
        .expect("seed rows");

    let service = DatabaseAdminService::new(Arc::clone(&pg));
    let ident = SafeIdentifier::parse(&table).expect("valid identifier");
    let (columns, row_count) = service.describe_table(&ident).await.expect("describe");

    drop_fixture_table(&pg, &table).await;

    assert_eq!(row_count, 2);
    assert_eq!(columns.len(), 3);

    let id = columns.iter().find(|c| c.name == "id").expect("id column");
    assert!(id.primary_key);
    assert!(!id.nullable);
    assert_eq!(id.data_type, "bigint");

    let label = columns
        .iter()
        .find(|c| c.name == "label")
        .expect("label column");
    assert!(!label.primary_key);
    assert!(!label.nullable);
    assert_eq!(label.default.as_deref(), Some("'x'::text"));

    let note = columns
        .iter()
        .find(|c| c.name == "note")
        .expect("note column");
    assert!(note.nullable);
    assert!(note.default.is_none());
}

#[tokio::test]
async fn describe_table_missing_table_is_not_found() {
    let Some(pg) = write_pool().await else { return };
    let service = DatabaseAdminService::new(pg);
    let ident = SafeIdentifier::parse("no_such_table_zzz").expect("valid identifier");

    let err = service.describe_table(&ident).await.expect_err("missing");
    assert!(matches!(err, RepositoryError::NotFound { .. }));
    assert!(err.to_string().contains("no_such_table_zzz"));
}

#[tokio::test]
async fn list_table_indexes_reports_pk_and_unique_index() {
    let Some(pg) = write_pool().await else { return };
    let table = unique_table();
    create_fixture_table(&pg, &table).await;
    let idx = format!("{table}_label_key");
    let ddl = format!("CREATE UNIQUE INDEX \"{idx}\" ON \"{table}\" (label, note)");
    sqlx::query(sqlx::AssertSqlSafe(ddl))
        .execute(&*pg)
        .await
        .expect("create index");

    let service = DatabaseAdminService::new(Arc::clone(&pg));
    let ident = SafeIdentifier::parse(&table).expect("valid identifier");
    let indexes = service.list_table_indexes(&ident).await.expect("indexes");

    drop_fixture_table(&pg, &table).await;

    assert_eq!(indexes.len(), 2);
    let unique = indexes.iter().find(|i| i.name == idx).expect("unique idx");
    assert!(unique.unique);
    assert_eq!(unique.columns, vec!["label".to_owned(), "note".to_owned()]);

    let pk = indexes
        .iter()
        .find(|i| i.name == format!("{table}_pkey"))
        .expect("pk index");
    assert!(pk.unique);
    assert_eq!(pk.columns, vec!["id".to_owned()]);
}

#[tokio::test]
async fn count_rows_counts_seeded_rows() {
    let Some(pg) = write_pool().await else { return };
    let table = unique_table();
    create_fixture_table(&pg, &table).await;
    let insert = format!("INSERT INTO \"{table}\" (id) SELECT generate_series(1, 7)");
    sqlx::query(sqlx::AssertSqlSafe(insert))
        .execute(&*pg)
        .await
        .expect("seed rows");

    let service = DatabaseAdminService::new(Arc::clone(&pg));
    let ident = SafeIdentifier::parse(&table).expect("valid identifier");
    let count = service.count_rows(&ident).await.expect("count");

    drop_fixture_table(&pg, &table).await;

    assert_eq!(count, 7);
}

#[tokio::test]
async fn list_expected_tables_names_core_schema() {
    let expected = DatabaseAdminService::list_expected_tables();
    assert!(expected.contains(&"users"));
    assert!(expected.contains(&"agent_tasks"));
    assert!(expected.contains(&"oauth_clients"));
    assert!(expected.len() >= 20);
}

fn swap_db_name(url: &str, new_db: &str) -> String {
    let (base, _old) = url.rsplit_once('/').expect("url has a database segment");
    format!("{base}/{new_db}")
}

#[tokio::test]
async fn database_info_lists_tables_with_rows_and_sizes_in_isolated_db() {
    let Some(admin_url) = fixture_database_url().ok() else {
        return;
    };
    let Ok(admin) = Database::new_postgres(&admin_url).await else {
        return;
    };
    let admin_pool = admin.write_pool_arc().expect("admin pool");

    let iso_db = format!("admin_info_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{iso_db}\"")))
        .execute(&*admin_pool)
        .await
        .expect("create isolated database");

    let iso_url = swap_db_name(&admin_url, &iso_db);
    let result = run_info_assertions(&iso_url).await;

    let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS \"{iso_db}\" WITH (FORCE)"
    )))
    .execute(&*admin_pool)
    .await;

    result.expect("database info assertions");
}

async fn run_info_assertions(iso_url: &str) -> anyhow::Result<()> {
    let db = Database::new_postgres(iso_url).await?;
    let pg = db.write_pool_arc()?;
    sqlx::query("CREATE TABLE info_probe (id BIGINT PRIMARY KEY)")
        .execute(&*pg)
        .await?;
    sqlx::query("INSERT INTO info_probe (id) VALUES (1), (2)")
        .execute(&*pg)
        .await?;

    let service = DatabaseAdminService::new(Arc::clone(&pg));

    let tables = service.list_tables().await?;
    let probe = tables
        .iter()
        .find(|t| t.name == "info_probe")
        .ok_or_else(|| anyhow::anyhow!("info_probe not listed"))?;
    anyhow::ensure!(probe.size_bytes > 0, "size_bytes should be positive");
    anyhow::ensure!(probe.columns.is_empty(), "list_tables omits columns");

    let info = service.get_database_info().await?;
    anyhow::ensure!(info.version.contains("PostgreSQL"), "version string");
    anyhow::ensure!(info.size > 0, "database size");
    anyhow::ensure!(info.path == "PostgreSQL", "path label");
    anyhow::ensure!(
        info.tables.iter().any(|t| t.name == "info_probe"),
        "tables included in info"
    );
    Ok(())
}
