//! DB-backed tests for the generic transaction wrappers in
//! `services/transaction.rs`: commit, rollback-on-error, and the retry path.
//!
//! Each test creates a uniquely-named temporary table so parallel runs never
//! collide, and drops it on the way out.

use std::sync::atomic::{AtomicU32, Ordering};

use super::db_helper::pool_or_skip;
use systemprompt_database::{
    DbPool, PgDbPool, RepositoryError, with_transaction, with_transaction_retry,
};

fn pg(db: &DbPool) -> PgDbPool {
    db.write_pool_arc().expect("write pool")
}

fn unique_table() -> String {
    format!("tx_test_{}", uuid::Uuid::new_v4().simple())
}

async fn create_table(pool: &sqlx::PgPool, table: &str) {
    let ddl = format!("CREATE TABLE \"{table}\" (id INT PRIMARY KEY)");
    sqlx::query(sqlx::AssertSqlSafe(ddl))
        .execute(pool)
        .await
        .expect("create table");
}

async fn drop_table(pool: &sqlx::PgPool, table: &str) {
    let ddl = format!("DROP TABLE IF EXISTS \"{table}\"");
    let _ = sqlx::query(sqlx::AssertSqlSafe(ddl)).execute(pool).await;
}

async fn row_count(pool: &sqlx::PgPool, table: &str) -> i64 {
    let q = format!("SELECT COUNT(*) FROM \"{table}\"");
    sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(q))
        .fetch_one(pool)
        .await
        .expect("count")
}

#[tokio::test]
async fn with_transaction_commits_inserted_rows() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let pool = pg(&db);
    let table = unique_table();
    create_table(&pool, &table).await;

    let table_for_closure = table.clone();
    let result: Result<i32, sqlx::Error> = with_transaction(&pool, move |tx| {
        let table = table_for_closure.clone();
        Box::pin(async move {
            let stmt = format!("INSERT INTO \"{table}\" (id) VALUES (1), (2)");
            sqlx::query(sqlx::AssertSqlSafe(stmt))
                .execute(&mut **tx)
                .await?;
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
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let pool = pg(&db);
    let table = unique_table();
    create_table(&pool, &table).await;

    let table_for_closure = table.clone();
    let result: Result<(), sqlx::Error> = with_transaction(&pool, move |tx| {
        let table = table_for_closure.clone();
        Box::pin(async move {
            let stmt = format!("INSERT INTO \"{table}\" (id) VALUES (1)");
            sqlx::query(sqlx::AssertSqlSafe(stmt.clone()))
                .execute(&mut **tx)
                .await?;
            // Force a unique-violation: same primary key twice.
            sqlx::query(sqlx::AssertSqlSafe(stmt))
                .execute(&mut **tx)
                .await?;
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
async fn with_transaction_commits_against_a_borrowed_pgpool() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let pool = pg(&db);
    let table = unique_table();
    create_table(&pool, &table).await;

    let table_for_closure = table.clone();
    let result: Result<(), sqlx::Error> = with_transaction(&pool, move |tx| {
        let table = table_for_closure.clone();
        Box::pin(async move {
            let stmt = format!("INSERT INTO \"{table}\" (id) VALUES (10)");
            sqlx::query(sqlx::AssertSqlSafe(stmt))
                .execute(&mut **tx)
                .await?;
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
    let Some(db) = pool_or_skip().await else {
        return;
    };
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
            sqlx::query(sqlx::AssertSqlSafe(stmt))
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
    let Some(db) = pool_or_skip().await else {
        return;
    };
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
                sqlx::query(sqlx::AssertSqlSafe(stmt))
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

#[tokio::test]
async fn a_real_deadlock_classifies_as_a_serialization_failure() {
    let Some(db) = pool_or_skip().await else {
        return;
    };
    let pool = pg(&db);
    let key_a = i64::from(uuid::Uuid::new_v4().as_u128() as u32) + 1;
    let key_b = key_a + 1;

    let mut first = pool.begin().await.expect("first transaction");
    let mut second = pool.begin().await.expect("second transaction");
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(key_a)
        .execute(&mut *first)
        .await
        .expect("first holds a");
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(key_b)
        .execute(&mut *second)
        .await
        .expect("second holds b");

    let first_waits = tokio::spawn(async move {
        let outcome = sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(key_b)
            .execute(&mut *first)
            .await;
        (first, outcome)
    });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let second_outcome = sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(key_a)
        .execute(&mut *second)
        .await;
    let (first, first_outcome) = first_waits.await.expect("join");

    let errors = [first_outcome, second_outcome]
        .into_iter()
        .filter_map(Result::err)
        .map(RepositoryError::from)
        .collect::<Vec<_>>();
    assert_eq!(
        errors.len(),
        1,
        "postgres aborts exactly one side of a deadlock"
    );
    assert!(
        errors[0].is_serialization_failure(),
        "a deadlock (40P01) is a retriable serialization failure, got {}",
        errors[0]
    );
    assert!(
        !RepositoryError::from(sqlx::Error::RowNotFound).is_serialization_failure(),
        "a non-database error is never a serialization failure"
    );
    drop(first);
    drop(second);
}
