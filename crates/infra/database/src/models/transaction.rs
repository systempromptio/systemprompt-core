//! Dyn-safe transaction trait used by the dynamic-SQL surface.

use crate::models::{JsonRow, QuerySelector, ToDbValue};
use anyhow::Result;
use async_trait::async_trait;

/// Active transaction handle returned by
/// [`crate::services::DatabaseProvider::begin_transaction`].
///
/// The trait is `dyn`-used (callers hold `Box<dyn DatabaseTransaction>`), so
/// `#[async_trait]` is required for object safety.
#[async_trait]
pub trait DatabaseTransaction: Send {
    /// Execute a non-returning statement (`INSERT` / `UPDATE` / `DELETE`) and
    /// return the affected row count.
    async fn execute(
        &mut self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<u64>;

    /// Fetch all matching rows.
    async fn fetch_all(
        &mut self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<Vec<JsonRow>>;

    /// Fetch a single row (errors if zero or more than one match).
    async fn fetch_one(
        &mut self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<JsonRow>;

    /// Fetch zero-or-one row.
    async fn fetch_optional(
        &mut self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> Result<Option<JsonRow>>;

    /// Commit the transaction, consuming it.
    async fn commit(self: Box<Self>) -> Result<()>;

    /// Roll back the transaction, consuming it.
    async fn rollback(self: Box<Self>) -> Result<()>;
}
