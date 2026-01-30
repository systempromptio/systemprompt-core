use anyhow::Result;
use async_trait::async_trait;
use std::any::Any;

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

    #[must_use]
    pub const fn content_id(&self) -> &str {
        self.content_id
    }

    #[must_use]
    pub const fn slug(&self) -> &str {
        self.slug
    }

    #[must_use]
    pub const fn source_name(&self) -> &str {
        self.source_name
    }

    #[must_use]
    pub const fn raw_frontmatter(&self) -> &serde_yaml::Value {
        self.raw_frontmatter
    }

    #[must_use]
    pub fn db_pool<T: 'static>(&self) -> Option<&T> {
        self.db_pool.downcast_ref::<T>()
    }
}

#[async_trait]
pub trait FrontmatterProcessor: Send + Sync {
    fn processor_id(&self) -> &'static str;

    fn applies_to_sources(&self) -> Vec<String> {
        vec![]
    }

    async fn process_frontmatter(&self, ctx: &FrontmatterContext<'_>) -> Result<()>;

    fn priority(&self) -> u32 {
        100
    }
}
