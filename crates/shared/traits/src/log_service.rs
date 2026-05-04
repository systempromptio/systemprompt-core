//! Generic log persistence trait.

use async_trait::async_trait;

/// Persist and query structured log entries.
///
/// The trait is generic over `Entry`, `Filter`, and `Error` so that each
/// backend can pick its own row type while presenting a uniform interface.
/// `#[async_trait]` is required because the trait is consumed as
/// `dyn LogService<...>` in dynamically dispatched contexts.
#[async_trait]
pub trait LogService: Send + Sync {
    /// Concrete log row type produced and consumed by the backend.
    type Entry: Send + Sync;
    /// Filter shape used by [`Self::query`].
    type Filter: Send + Sync;
    /// Error returned by every fallible method.
    type Error: std::error::Error + Send + Sync;

    /// Persist a single log entry.
    async fn log(&self, entry: Self::Entry) -> Result<(), Self::Error>;

    /// Run a paginated filtered query, returning matching entries plus the
    /// total row count.
    async fn query(&self, filter: &Self::Filter) -> Result<(Vec<Self::Entry>, i64), Self::Error>;

    /// Return the most recent `limit` entries.
    async fn get_recent(&self, limit: i64) -> Result<Vec<Self::Entry>, Self::Error>;

    /// Look up a single entry by id.
    async fn get_by_id(&self, id: &str) -> Result<Option<Self::Entry>, Self::Error>;

    /// Delete the entry identified by `id`, returning whether a row was
    /// removed.
    async fn delete(&self, id: &str) -> Result<bool, Self::Error>;
}
