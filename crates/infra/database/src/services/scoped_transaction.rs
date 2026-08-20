//! Transaction wrappers that apply per-request connection scope (custom GUCs
//! for row-level security) after `BEGIN`.
//!
//! `SET LOCAL` semantics: every setting applied via `set_config(.., true)`
//! dies at COMMIT or ROLLBACK, so the connection returns to the pool clean —
//! no cross-request leakage, even on panic (dropping the transaction rolls
//! back). With no registered [`crate::scope::ConnectionScopeProvider`] these
//! wrappers are a plain `pool.begin()`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{PgPool, Postgres, Transaction};
use systemprompt_models::RequestScope;

use crate::error::RepositoryError;
use crate::repository::PgDbPool;
use crate::scope::scope_providers;

use super::transaction::BoxFuture;

pub async fn begin_scoped(
    pool: &PgPool,
    scope: &RequestScope,
) -> Result<Transaction<'static, Postgres>, RepositoryError> {
    let mut tx = pool.begin().await?;
    let providers = scope_providers();
    if providers.is_empty() || scope.is_empty() {
        return Ok(tx);
    }
    for provider in providers {
        let settings = provider
            .scope_settings(scope)
            .await
            .map_err(|e| RepositoryError::InvalidArgument(e.to_string()))?;
        for setting in settings {
            sqlx::query_scalar!(
                "SELECT set_config($1, $2, true)",
                setting.key(),
                setting.value()
            )
            .fetch_one(&mut *tx)
            .await?;
        }
    }
    Ok(tx)
}

pub async fn with_scoped_transaction_raw<F, T, E>(
    pool: &PgPool,
    scope: &RequestScope,
    f: F,
) -> Result<T, E>
where
    F: for<'c> FnOnce(&'c mut Transaction<'_, Postgres>) -> BoxFuture<'c, Result<T, E>>,
    E: From<sqlx::Error> + From<RepositoryError>,
{
    let mut tx = begin_scoped(pool, scope).await?;
    let result = f(&mut tx).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn with_scoped_transaction<F, T, E>(
    pool: &PgDbPool,
    scope: &RequestScope,
    f: F,
) -> Result<T, E>
where
    F: for<'c> FnOnce(&'c mut Transaction<'_, Postgres>) -> BoxFuture<'c, Result<T, E>>,
    E: From<sqlx::Error> + From<RepositoryError>,
{
    with_scoped_transaction_raw(pool, scope, f).await
}
