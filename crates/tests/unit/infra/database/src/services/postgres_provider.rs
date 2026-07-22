//! DB-backed tests for `PostgresProvider`: the `DatabaseProvider` trait
//! surface, `PostgresTransaction`, and the typed `DatabaseProviderExt`
//! fetch helpers. Each test uses a uniquely-named temp table.

use std::sync::Arc;

use super::db_helper::pool;
use systemprompt_database::{
    DatabaseProvider, DatabaseProviderExt, DatabaseResult, DbValue, FromDatabaseRow,
    PostgresProvider, RepositoryError,
};
use systemprompt_test_fixtures::fixture_database_url;

async fn provider() -> Option<PostgresProvider> {
    let db = pool().await?;
    let pg = db.write_pool_arc().ok()?;
    Some(PostgresProvider::from_pool(pg))
}

fn unique_table() -> String {
    format!("pg_provider_{}", uuid::Uuid::new_v4().simple())
}

async fn create_table(provider: &PostgresProvider, table: &str) {
    let ddl =
        format!("CREATE TABLE \"{table}\" (id BIGINT PRIMARY KEY, name TEXT, active BOOLEAN)");
    provider.execute_raw(&ddl).await.expect("create table");
}

async fn drop_table(provider: &PostgresProvider, table: &str) {
    let ddl = format!("DROP TABLE IF EXISTS \"{table}\"");
    let _ = provider.execute_raw(&ddl).await;
}

#[tokio::test]
async fn execute_binds_params_and_reports_rows_affected() {
    let Some(provider) = provider().await else {
        return;
    };
    let table = unique_table();
    create_table(&provider, &table).await;

    let insert = format!("INSERT INTO \"{table}\" (id, name, active) VALUES ($1, $2, $3)");
    let affected = provider
        .execute(&insert, &[&7_i64, &"seven".to_owned(), &true])
        .await
        .expect("insert");
    assert_eq!(affected, 1);

    let update = format!("UPDATE \"{table}\" SET active = $1 WHERE id = $2");
    let affected = provider
        .execute(&update, &[&false, &7_i64])
        .await
        .expect("update");
    assert_eq!(affected, 1);

    drop_table(&provider, &table).await;
}

#[tokio::test]
async fn fetch_one_all_and_optional_round_trip_rows() {
    let Some(provider) = provider().await else {
        return;
    };
    let table = unique_table();
    create_table(&provider, &table).await;
    let insert = format!(
        "INSERT INTO \"{table}\" (id, name, active) VALUES (1, 'a', true), (2, NULL, false)"
    );
    provider.execute_raw(&insert).await.expect("seed");

    let select_all = format!("SELECT id, name, active FROM \"{table}\" ORDER BY id");
    let rows = provider.fetch_all(&select_all, &[]).await.expect("all");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], serde_json::json!(1));
    assert_eq!(rows[0]["name"], serde_json::json!("a"));
    assert_eq!(rows[1]["name"], serde_json::Value::Null);
    assert_eq!(rows[1]["active"], serde_json::json!(false));

    let select_one = format!("SELECT name FROM \"{table}\" WHERE id = $1");
    let row = provider
        .fetch_one(&select_one, &[&1_i64])
        .await
        .expect("one");
    assert_eq!(row["name"], serde_json::json!("a"));

    let found = provider
        .fetch_optional(&select_one, &[&2_i64])
        .await
        .expect("optional some");
    assert!(found.is_some());

    let missing = provider
        .fetch_optional(&select_one, &[&99_i64])
        .await
        .expect("optional none");
    assert!(missing.is_none());

    drop_table(&provider, &table).await;
}

#[tokio::test]
async fn fetch_scalar_value_maps_json_types_to_db_values() {
    let Some(provider) = provider().await else {
        return;
    };

    let s = provider
        .fetch_scalar_value(&"SELECT 'hello'::text", &[])
        .await
        .expect("string scalar");
    assert!(matches!(s, DbValue::String(v) if v == "hello"));

    let i = provider
        .fetch_scalar_value(&"SELECT 42::bigint", &[])
        .await
        .expect("int scalar");
    assert!(matches!(i, DbValue::Int(42)));

    let f = provider
        .fetch_scalar_value(&"SELECT 1.5::float8", &[])
        .await
        .expect("float scalar");
    assert!(matches!(f, DbValue::Float(v) if (v - 1.5).abs() < f64::EPSILON));

    let b = provider
        .fetch_scalar_value(&"SELECT true", &[])
        .await
        .expect("bool scalar");
    assert!(matches!(b, DbValue::Bool(true)));

    let n = provider
        .fetch_scalar_value(&"SELECT NULL::text", &[])
        .await
        .expect("null scalar");
    assert!(matches!(n, DbValue::NullString));

    let err = provider
        .fetch_scalar_value(&"SELECT ARRAY['a','b']", &[])
        .await
        .expect_err("array scalar unsupported");
    assert!(matches!(err, RepositoryError::InvalidState { .. }));
}

#[tokio::test]
async fn query_raw_and_query_raw_with_report_columns_and_counts() {
    let Some(provider) = provider().await else {
        return;
    };

    let result = provider
        .query_raw(&"SELECT generate_series(1, 3) AS n")
        .await
        .expect("query_raw");
    assert_eq!(result.columns, vec!["n".to_owned()]);
    assert_eq!(result.row_count, 3);
    assert_eq!(result.rows.len(), 3);

    let result = provider
        .query_raw_with(&"SELECT $1::text AS greeting", &[&"hi".to_owned()])
        .await
        .expect("query_raw_with");
    assert_eq!(result.columns, vec!["greeting".to_owned()]);
    assert_eq!(result.rows[0]["greeting"], serde_json::json!("hi"));

    let empty = provider
        .query_raw(&"SELECT 1 AS n WHERE false")
        .await
        .expect("empty result");
    assert!(empty.is_empty());
    assert!(empty.columns.is_empty());
}

#[tokio::test]
async fn execute_batch_runs_each_statement() {
    let Some(provider) = provider().await else {
        return;
    };
    let table = unique_table();

    let batch = format!(
        "CREATE TABLE \"{table}\" (id BIGINT PRIMARY KEY);\nINSERT INTO \"{table}\" (id) VALUES \
         (1);\nINSERT INTO \"{table}\" (id) VALUES (2);"
    );
    provider.execute_batch(&batch).await.expect("batch");

    let count = provider
        .fetch_scalar_value(&format!("SELECT COUNT(*) FROM \"{table}\""), &[])
        .await
        .expect("count");
    assert!(matches!(count, DbValue::Int(2)));

    drop_table(&provider, &table).await;
}

#[tokio::test]
async fn test_connection_succeeds_and_pool_accessors_expose_postgres() {
    let Some(provider) = provider().await else {
        return;
    };
    provider.test_connection().await.expect("connection probe");
    assert!(provider.is_postgres());
    assert!(provider.get_postgres_pool().is_some());
}

#[tokio::test]
async fn transaction_commit_persists_and_rollback_discards() {
    let Some(provider) = provider().await else {
        return;
    };
    let table = unique_table();
    create_table(&provider, &table).await;
    let insert = format!("INSERT INTO \"{table}\" (id, name) VALUES ($1, $2)");
    let count_sql = format!("SELECT COUNT(*) FROM \"{table}\"");

    let mut tx = provider.begin_transaction().await.expect("begin");
    let affected = tx
        .execute(&insert, &[&1_i64, &"committed".to_owned()])
        .await
        .expect("tx insert");
    assert_eq!(affected, 1);
    tx.commit().await.expect("commit");

    let mut tx = provider.begin_transaction().await.expect("begin 2");
    tx.execute(&insert, &[&2_i64, &"discarded".to_owned()])
        .await
        .expect("tx insert 2");
    let inside = tx
        .fetch_one(&count_sql, &[])
        .await
        .expect("count inside tx");
    assert_eq!(inside["count"], serde_json::json!(2));
    tx.rollback().await.expect("rollback");

    let count = provider
        .fetch_scalar_value(&count_sql, &[])
        .await
        .expect("count after rollback");
    assert!(matches!(count, DbValue::Int(1)));

    drop_table(&provider, &table).await;
}

#[tokio::test]
async fn transaction_fetch_variants_see_uncommitted_rows() {
    let Some(provider) = provider().await else {
        return;
    };
    let table = unique_table();
    create_table(&provider, &table).await;

    let mut tx = provider.begin_transaction().await.expect("begin");
    let insert = format!("INSERT INTO \"{table}\" (id, name) VALUES (5, 'tx-only')");
    tx.execute(&insert, &[]).await.expect("tx insert");

    let select = format!("SELECT id, name FROM \"{table}\" WHERE id = $1");
    let rows = tx.fetch_all(&select, &[&5_i64]).await.expect("tx all");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], serde_json::json!("tx-only"));

    let some = tx
        .fetch_optional(&select, &[&5_i64])
        .await
        .expect("tx optional some");
    assert!(some.is_some());
    let none = tx
        .fetch_optional(&select, &[&6_i64])
        .await
        .expect("tx optional none");
    assert!(none.is_none());

    tx.rollback().await.expect("rollback");

    let outside = provider
        .fetch_optional(&select, &[&5_i64])
        .await
        .expect("outside");
    assert!(outside.is_none());

    drop_table(&provider, &table).await;
}

struct NamedRow {
    id: i64,
    name: Option<String>,
}

impl FromDatabaseRow for NamedRow {
    fn from_postgres_row(row: &sqlx::postgres::PgRow) -> DatabaseResult<Self> {
        use sqlx::Row;
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
        })
    }
}

#[tokio::test]
async fn typed_fetch_helpers_decode_rows() {
    let Some(provider) = provider().await else {
        return;
    };
    let table = unique_table();
    create_table(&provider, &table).await;
    let insert = format!("INSERT INTO \"{table}\" (id, name) VALUES (1, 'a'), (2, NULL)");
    provider.execute_raw(&insert).await.expect("seed");

    let by_id = format!("SELECT id, name FROM \"{table}\" WHERE id = $1");
    let one: NamedRow = provider
        .fetch_typed_one(&by_id, &[&1_i64])
        .await
        .expect("typed one");
    assert_eq!(one.id, 1);
    assert_eq!(one.name.as_deref(), Some("a"));

    let some: Option<NamedRow> = provider
        .fetch_typed_optional(&by_id, &[&2_i64])
        .await
        .expect("typed optional some");
    assert!(some.is_some_and(|r| r.name.is_none()));

    let none: Option<NamedRow> = provider
        .fetch_typed_optional(&by_id, &[&9_i64])
        .await
        .expect("typed optional none");
    assert!(none.is_none());

    let all: Vec<NamedRow> = provider
        .fetch_typed_all(
            &format!("SELECT id, name FROM \"{table}\" ORDER BY id"),
            &[],
        )
        .await
        .expect("typed all");
    assert_eq!(all.len(), 2);
    assert_eq!(all[1].id, 2);

    drop_table(&provider, &table).await;
}

#[tokio::test]
async fn new_connects_with_explicit_sslmode_disable() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let url = if url.contains('?') {
        format!("{url}&sslmode=disable")
    } else {
        format!("{url}?sslmode=disable")
    };

    let provider = PostgresProvider::new(&url).await.expect("connect");
    provider.test_connection().await.expect("probe");
}
