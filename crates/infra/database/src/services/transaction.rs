use crate::error::RepositoryError;
use crate::repository::PgDbPool;
use sqlx::{PgPool, Postgres, Transaction};
use std::future::Future;
use std::pin::Pin;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub async fn with_transaction<F, T, E>(pool: &PgDbPool, f: F) -> Result<T, E>
where
    F: for<'c> FnOnce(&'c mut Transaction<'_, Postgres>) -> BoxFuture<'c, Result<T, E>>,
    E: From<sqlx::Error>,
{
    let mut tx = pool.begin().await?;
    let result = f(&mut tx).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn with_transaction_raw<F, T, E>(pool: &PgPool, f: F) -> Result<T, E>
where
    F: for<'c> FnOnce(&'c mut Transaction<'_, Postgres>) -> BoxFuture<'c, Result<T, E>>,
    E: From<sqlx::Error>,
{
    let mut tx = pool.begin().await?;
    let result = f(&mut tx).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn with_transaction_retry<F, T>(
    pool: &PgDbPool,
    max_retries: u32,
    f: F,
) -> Result<T, RepositoryError>
where
    F: for<'c> Fn(&'c mut Transaction<'_, Postgres>) -> BoxFuture<'c, Result<T, RepositoryError>>
        + Send,
{
    let mut attempts = 0;
    let base_delay_ms = 10u64;

    loop {
        let mut tx = pool.begin().await?;

        match f(&mut tx).await {
            Ok(result) => {
                tx.commit().await?;
                return Ok(result);
            },
            Err(e) => {
                if attempts < max_retries && is_retriable_error(&e) {
                    if let Err(rollback_err) = tx.rollback().await {
                        tracing::error!(error = %rollback_err, "Transaction rollback failed during retry");
                    }
                    attempts += 1;
                    let delay_ms = base_delay_ms * (1 << attempts.min(6));
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    continue;
                }
                if let Err(rollback_err) = tx.rollback().await {
                    tracing::error!(error = %rollback_err, "Transaction rollback failed");
                }
                return Err(e);
            },
        }
    }
}

fn is_retriable_error(error: &RepositoryError) -> bool {
    match error {
        RepositoryError::Database(sqlx_error) => {
            sqlx_error.as_database_error().is_some_and(|db_error| {
                let code = db_error.code().map(|c| c.to_string());
                matches!(code.as_deref(), Some("40001" | "40P01"))
            })
        },
        RepositoryError::NotFound(_)
        | RepositoryError::Constraint(_)
        | RepositoryError::Serialization(_)
        | RepositoryError::InvalidArgument(_)
        | RepositoryError::Internal(_) => false,
    }
}
