//! DB-backed tests for the `Database` handle: config construction, the
//! read/write delegation surface, and the boot-path validation helpers.

use systemprompt_database::services::DatabaseProvider;
use systemprompt_database::{
    Database, PoolConfig, validate_column_exists, validate_database_connection,
    validate_table_exists,
};
use systemprompt_test_fixtures::fixture_database_url;

fn pool_config() -> PoolConfig {
    PoolConfig {
        max_connections: 4,
        min_connections: 0,
        acquire_timeout: std::time::Duration::from_secs(30),
        idle_timeout: std::time::Duration::from_secs(30),
        max_lifetime: std::time::Duration::from_secs(300),
    }
}

async fn database() -> Option<Database> {
    let url = fixture_database_url().ok()?;
    Database::from_config("postgres", &url).await.ok()
}

fn unique_table() -> String {
    format!("db_surface_{}", uuid::Uuid::new_v4().simple())
}

#[tokio::test]
async fn from_config_rejects_unsupported_backend() {
    let err = Database::from_config("mysql", "mysql://x").await;
    assert!(err.is_err());

    let err = Database::from_config_with_write("sqlite", "u", None, &pool_config()).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn from_config_with_write_builds_distinct_write_provider() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let db = Database::from_config_with_write("postgres", &url, Some(&url), &pool_config())
        .await
        .expect("connects");

    assert!(db.has_write_pool());
    assert!(db.write_pool().is_some());
    db.test_connection().await.expect("both pools reachable");

    let tx = db.begin().await.expect("begin against write pool");
    tx.rollback().await.expect("rollback");
}

#[tokio::test]
async fn debug_and_info_report_postgres_backend() {
    let Some(db) = database().await else {
        return;
    };
    assert!(format!("{db:?}").contains("PostgreSQL"));

    let info = db.get_info().await.expect("info");
    assert!(!info.version.is_empty());
}

#[tokio::test]
async fn provider_impl_delegates_reads_and_writes() {
    let Some(db) = database().await else {
        return;
    };
    let table = unique_table();
    db.execute_batch(&format!(
        "CREATE TABLE \"{table}\" (id BIGINT PRIMARY KEY, name TEXT NOT NULL); INSERT INTO \
         \"{table}\" (id, name) VALUES (1, 'one');"
    ))
    .await
    .expect("create+seed");

    let insert = format!("INSERT INTO \"{table}\" (id, name) VALUES ($1, $2)");
    let affected = DatabaseProvider::execute(&db, &insert, &[&2_i64, &"two".to_owned()])
        .await
        .expect("insert");
    assert_eq!(affected, 1);

    let select_one = format!("SELECT name FROM \"{table}\" WHERE id = $1");
    let row = db.fetch_one(&select_one, &[&1_i64]).await.expect("row");
    assert_eq!(
        row.get("name").and_then(serde_json::Value::as_str),
        Some("one")
    );

    let missing = db
        .fetch_optional(&select_one, &[&99_i64])
        .await
        .expect("optional");
    assert!(missing.is_none());

    let select_ids = format!("SELECT id FROM \"{table}\"");
    let all = db.fetch_all(&select_ids, &[]).await.expect("all");
    assert_eq!(all.len(), 2);

    let count_sql = format!("SELECT COUNT(*) FROM \"{table}\" WHERE id > $1");
    let scalar = db
        .fetch_scalar_value(&count_sql, &[&0_i64])
        .await
        .expect("scalar");
    assert!(format!("{scalar:?}").contains('2'));

    let ordered = format!("SELECT id, name FROM \"{table}\" ORDER BY id");
    let raw = db.query_raw(&ordered).await.expect("raw");
    assert_eq!(raw.row_count, 2);

    let raw_with = db
        .query_raw_with(&select_one, &[&2_i64])
        .await
        .expect("raw with");
    assert_eq!(raw_with.row_count, 1);

    DatabaseProvider::test_connection(&db).await.expect("ping");
    let info = db.get_database_info().await.expect("info");
    assert!(!info.version.is_empty());

    let mut tx = db.begin_transaction().await.expect("tx");
    let affected = tx
        .execute(&insert, &[&3_i64, &"three".to_owned()])
        .await
        .expect("tx insert");
    assert_eq!(affected, 1);
    tx.rollback().await.expect("rollback");

    let remaining = db
        .fetch_optional(&select_one, &[&3_i64])
        .await
        .expect("optional");
    assert!(remaining.is_none(), "rolled-back row must not persist");

    db.execute_raw(&format!("DROP TABLE \"{table}\""))
        .await
        .expect("drop");
}

#[tokio::test]
async fn validation_helpers_detect_tables_and_columns() {
    let Some(db) = database().await else {
        return;
    };

    validate_database_connection(&db)
        .await
        .expect("connection valid");

    assert!(validate_table_exists(&db, "users").await.expect("query"));
    assert!(
        !validate_table_exists(&db, "definitely_not_a_table_xyz")
            .await
            .expect("query")
    );

    assert!(
        validate_column_exists(&db, "users", "id")
            .await
            .expect("query")
    );
    assert!(
        !validate_column_exists(&db, "users", "no_such_column_xyz")
            .await
            .expect("query")
    );
}
