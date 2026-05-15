//! Generic transaction wrappers that work directly with [`PgPool`] /
//! [`PgDbPool`] without going through the dyn-safe trait.

use crate::error::RepositoryError;
use crate::repository::PgDbPool;
use crate::resilience::classify::Outcome;
use crate::resilience::config::RetryConfig;
use crate::resilience::retry::retry_async;
use sqlx::{PgPool, Postgres, Transaction};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

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
    T: Send,
    F: for<'c> Fn(&'c mut Transaction<'_, Postgres>) -> BoxFuture<'c, Result<T, RepositoryError>>
        + Send
        + Sync,
{
    let cfg = RetryConfig {
        max_attempts: max_retries.saturating_add(1),
        base_delay: Duration::from_millis(20),
        max_delay: Duration::from_millis(640),
        jitter: false,
    };
    let classify = |err: &RepositoryError| {
        if is_retriable_error(err) {
            Outcome::Transient { retry_after: None }
        } else {
            Outcome::Permanent
        }
    };
    let attempt = || async {
        let mut tx = pool.begin().await?;
        match f(&mut tx).await {
            Ok(result) => {
                tx.commit().await?;
                Ok(result)
            },
            Err(e) => {
                if let Err(rollback_err) = tx.rollback().await {
                    tracing::error!(error = %rollback_err, "Transaction rollback failed");
                }
                Err(e)
            },
        }
    };
    retry_async(&cfg, "transaction", classify, attempt).await
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
        | RepositoryError::InvalidState(_)
        | RepositoryError::Internal(_) => false,
    }
}
