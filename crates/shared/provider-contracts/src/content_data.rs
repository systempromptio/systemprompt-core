//! [`ContentDataProvider`] contract for enriching content items with extra
//! data joined from outside the source file (database lookups, etc.).

use async_trait::async_trait;
use serde_json::Value;
use std::any::Any;

use crate::error::ProviderResult;

/// Per-call context handed to a [`ContentDataProvider`].
pub struct ContentDataContext<'a> {
    content_id: &'a str,
    source_name: &'a str,
    db_pool: &'a (dyn Any + Send + Sync),
}

impl std::fmt::Debug for ContentDataContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentDataContext")
            .field("content_id", &self.content_id)
            .field("source_name", &self.source_name)
            .field("db_pool", &"<dyn Any>")
            .finish()
    }
}

impl<'a> ContentDataContext<'a> {
    /// Build a [`ContentDataContext`] from its parts.
    #[must_use]
    pub fn new(
        content_id: &'a str,
        source_name: &'a str,
        db_pool: &'a (dyn Any + Send + Sync),
    ) -> Self {
        Self {
            content_id,
            source_name,
            db_pool,
        }
    }

    /// Stable identifier of the content item being enriched.
    #[must_use]
    pub const fn content_id(&self) -> &str {
        self.content_id
    }

    /// Logical content source name (e.g. `blog`, `docs`).
    #[must_use]
    pub const fn source_name(&self) -> &str {
        self.source_name
    }

    /// Type-erased downcast to the host's database pool.
    #[must_use]
    pub fn db_pool<T: 'static>(&self) -> Option<&T> {
        self.db_pool.downcast_ref::<T>()
    }
}

/// Hook invoked once per content item to enrich it with external data.
///
/// Marked `#[async_trait]` because it is consumed via
/// `dyn ContentDataProvider`.
#[async_trait]
pub trait ContentDataProvider: Send + Sync {
    /// Stable identifier for this provider.
    fn provider_id(&self) -> &'static str;

    /// Source names this provider opts into; empty means "all".
    fn applies_to_sources(&self) -> Vec<String> {
        vec![]
    }

    /// Mutate `item` to attach the provider's enriched data.
    async fn enrich_content(
        &self,
        ctx: &ContentDataContext<'_>,
        item: &mut Value,
    ) -> ProviderResult<()>;

    /// Provider priority; higher runs first.
    fn priority(&self) -> u32 {
        100
    }
}
