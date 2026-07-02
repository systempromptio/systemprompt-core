//! Generic log persistence trait.
//!
//! Dispatched as a trait object (`dyn _`), so it uses `#[async_trait]`;
//! native `async fn` in traits is not yet `dyn`-compatible.

use async_trait::async_trait;

#[async_trait]
pub trait LogService: Send + Sync {
    type Entry: Send + Sync;
    type Filter: Send + Sync;
    type Error: std::error::Error + Send + Sync;

    async fn log(&self, entry: Self::Entry) -> Result<(), Self::Error>;

    async fn query(&self, filter: &Self::Filter) -> Result<(Vec<Self::Entry>, i64), Self::Error>;

    async fn list_recent(&self, limit: i64) -> Result<Vec<Self::Entry>, Self::Error>;

    async fn find_by_id(&self, id: &str) -> Result<Option<Self::Entry>, Self::Error>;

    async fn delete(&self, id: &str) -> Result<bool, Self::Error>;
}
