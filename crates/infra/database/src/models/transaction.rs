//! Dyn-safe transaction trait used by the dynamic-SQL surface.

use crate::error::DatabaseResult;
use crate::models::{JsonRow, QuerySelector, ToDbValue};
use async_trait::async_trait;

#[async_trait]
pub trait DatabaseTransaction: Send {
    async fn execute(
        &mut self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<u64>;

    async fn fetch_all(
        &mut self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Vec<JsonRow>>;

    async fn fetch_one(
        &mut self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<JsonRow>;

    async fn fetch_optional(
        &mut self,
        query: &dyn QuerySelector,
        params: &[&dyn ToDbValue],
    ) -> DatabaseResult<Option<JsonRow>>;

    async fn commit(self: Box<Self>) -> DatabaseResult<()>;

    async fn rollback(self: Box<Self>) -> DatabaseResult<()>;
}
