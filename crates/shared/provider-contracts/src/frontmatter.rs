//! [`FrontmatterProcessor`] contract for post-processing parsed frontmatter.

use async_trait::async_trait;
use std::any::Any;

use crate::error::ProviderResult;

/// Per-call context handed to a [`FrontmatterProcessor`].
pub struct FrontmatterContext<'a> {
    content_id: &'a str,
    slug: &'a str,
    source_name: &'a str,
    raw_frontmatter: &'a serde_yaml::Value,
    db_pool: &'a (dyn Any + Send + Sync),
}

impl std::fmt::Debug for FrontmatterContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrontmatterContext")
            .field("content_id", &self.content_id)
            .field("slug", &self.slug)
            .field("source_name", &self.source_name)
            .field("db_pool", &"<dyn Any>")
            .finish()
    }
}

impl<'a> FrontmatterContext<'a> {
    /// Build a [`FrontmatterContext`] from its parts.
    #[must_use]
    pub fn new(
        content_id: &'a str,
        slug: &'a str,
        source_name: &'a str,
        raw_frontmatter: &'a serde_yaml::Value,
        db_pool: &'a (dyn Any + Send + Sync),
    ) -> Self {
        Self {
            content_id,
            slug,
            source_name,
            raw_frontmatter,
            db_pool,
        }
    }

    /// Stable identifier of the content item being processed.
    #[must_use]
    pub const fn content_id(&self) -> &str {
        self.content_id
    }

    /// URL slug of the content item being processed.
    #[must_use]
    pub const fn slug(&self) -> &str {
        self.slug
    }

    /// Logical content source name (e.g. `blog`, `docs`).
    #[must_use]
    pub const fn source_name(&self) -> &str {
        self.source_name
    }

    /// Raw YAML frontmatter as parsed off disk.
    #[must_use]
    pub const fn raw_frontmatter(&self) -> &serde_yaml::Value {
        self.raw_frontmatter
    }

    /// Type-erased downcast to the host's database pool.
    #[must_use]
    pub fn db_pool<T: 'static>(&self) -> Option<&T> {
        self.db_pool.downcast_ref::<T>()
    }
}

/// Hook invoked once per content item during frontmatter ingestion.
///
/// Marked `#[async_trait]` because it is consumed via
/// `dyn FrontmatterProcessor`.
#[async_trait]
pub trait FrontmatterProcessor: Send + Sync {
    /// Stable identifier for this processor.
    fn processor_id(&self) -> &'static str;

    /// Source names this processor opts into; empty means "all".
    fn applies_to_sources(&self) -> Vec<String> {
        vec![]
    }

    /// Run the processor against `ctx`.
    async fn process_frontmatter(&self, ctx: &FrontmatterContext<'_>) -> ProviderResult<()>;

    /// Processor priority; higher runs first.
    fn priority(&self) -> u32 {
        100
    }
}
