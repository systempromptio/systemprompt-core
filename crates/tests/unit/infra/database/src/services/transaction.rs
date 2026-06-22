//! DB-backed tests for the generic transaction wrappers in
//! `services/transaction.rs`: commit, rollback-on-error, and the retry path.
//!
//! Each test creates a uniquely-named temporary table so parallel runs never
//! collide, and drops it on the way out.

use std::sync::atomic::{AtomicU32, Ordering};

use super::db_helper::pool;
use systemprompt_database::{
    DbPool, PgDbPool, with_transaction, with_transaction_raw, with_transaction_retry,
};

fn pg(db: &DbPool) -> PgDbPool {
    db.write_pool_arc().expect("write pool")
}

fn unique_table() -> String {
    format!("tx_test_{}", uuid::Uuid::new_v4().simple())
}

async fn create_table(pool: &sqlx::PgPool, table: &str) {
    let ddl = format!("CREATE TABLE \"{table}\" (id INT PRIMARY KEY)");
    sqlx::query(&ddl).execute(pool).await.expect("create table");
}

async fn drop_table(pool: &sqlx::PgPool, table: &str) {
    let ddl = format!("DROP TABLE IF EXISTS \"{table}\"");
    let _ = sqlx::query(&ddl).execute(pool).await;
}

async fn row_count(pool: &sqlx::PgPool, table: &str) -> i64 {
    let q = format!("SELECT COUNT(*) FROM \"{table}\"");
    sqlx::query_scalar::<_, i64>(&q)
        .fetch_one(pool)
        .await
        .expect("count")
}

#[tokio::test]
async fn with_transaction_commits_inserted_rows() {
    let Some(db) = pool().await else { return };
    let pool = pg(&db);
    let table = unique_table();
    create_table(&pool, &table).await;

    let table_for_closure = table.clone();
    let result: Result<i32, sqlx::Error> = with_transaction(&pool, move |tx| {
        let table = table_for_closure.clone();
        Box::pin(async move {
            let stmt = format!("INSERT INTO \"{table}\" (id) VALUES (1), (2)");
            sqlx::query(&stmt).execute(&mut **tx).await?;
            Ok(7)
        })
    })
    .await;

    assert_eq!(result.expect("commit ok"), 7);
    assert_eq!(row_count(&pool, &table).await, 2);

    drop_table(&pool, &table).await;
}

#[tokio::test]
async fn with_transaction_rolls_back_on_closure_error() {
    let Some(db) = pool().await else { return };
    let pool = pg(&db);
    let table = unique_table();
    create_table(&pool, &table).await;

    let table_for_closure = table.clone();
    let result: Result<(), sqlx::Error> = with_transaction(&pool, move |tx| {
        let table = table_for_closure.clone();
        Box::pin(async move {
            let stmt = format!("INSERT INTO \"{table}\" (id) VALUES (1)");
            sqlx::query(&stmt).execute(&mut **tx).await?;
            // Force a unique-violation: same primary key twice.
            sqlx::query(&stmt).execute(&mut **tx).await?;
            Ok(())
        })
    })
    .await;

    assert!(result.is_err(), "duplicate PK must surface an error");
    assert_eq!(
        row_count(&pool, &table).await,
        0,
        "a failing transaction must leave no committed rows"
    );

    drop_table(&pool, &table).await;
}

#[tokio::test]
async fn with_transaction_raw_commits_against_pgpool() {
    let Some(db) = pool().await else { return };
    let pool = pg(&db);
    let table = unique_table();
    create_table(&pool, &table).await;

    let table_for_closure = table.clone();
    let result: Result<(), sqlx::Error> = with_transaction_raw(&pool, move |tx| {
        let table = table_for_closure.clone();
        Box::pin(async move {
            let stmt = format!("INSERT INTO \"{table}\" (id) VALUES (10)");
            sqlx::query(&stmt).execute(&mut **tx).await?;
            Ok(())
        })
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(row_count(&pool, &table).await, 1);

    drop_table(&pool, &table).await;
}

#[tokio::test]
async fn with_transaction_retry_commits_on_first_success() {
    let Some(db) = pool().await else { return };
    let pool = pg(&db);
    let table = unique_table();
    create_table(&pool, &table).await;

    let attempts = AtomicU32::new(0);
    let table_for_closure = table.clone();
    let result = with_transaction_retry(&pool, 3, |tx| {
        attempts.fetch_add(1, Ordering::SeqCst);
        let table = table_for_closure.clone();
        Box::pin(async move {
            let stmt = format!("INSERT INTO \"{table}\" (id) VALUES (5)");
            sqlx::query(&stmt)
                .execute(&mut **tx)
                .await
                .map_err(systemprompt_database::RepositoryError::from)?;
            Ok::<_, systemprompt_database::RepositoryError>(99)
        })
    })
    .await;

    assert_eq!(result.expect("ok"), 99);
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    assert_eq!(row_count(&pool, &table).await, 1);

    drop_table(&pool, &table).await;
}

#[tokio::test]
async fn with_transaction_retry_does_not_retry_permanent_error() {
    let Some(db) = pool().await else { return };
    let pool = pg(&db);
    let table = unique_table();
    create_table(&pool, &table).await;

    let attempts = AtomicU32::new(0);
    let table_for_closure = table.clone();
    let result: Result<(), systemprompt_database::RepositoryError> =
        with_transaction_retry(&pool, 3, |tx| {
            attempts.fetch_add(1, Ordering::SeqCst);
            let table = table_for_closure.clone();
            Box::pin(async move {
                let stmt = format!("INSERT INTO \"{table}\" (id) VALUES (1), (1)");
                sqlx::query(&stmt)
                    .execute(&mut **tx)
                    .await
                    .map_err(systemprompt_database::RepositoryError::from)?;
                Ok(())
            })
        })
        .await;

    assert!(result.is_err(), "unique violation is permanent");
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        1,
        "a non-serialization error (23505) must not be retried"
    );
    assert_eq!(row_count(&pool, &table).await, 0);

    drop_table(&pool, &table).await;
}
