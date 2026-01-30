use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::any::Any;

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

    #[must_use]
    pub const fn content_id(&self) -> &str {
        self.content_id
    }

    #[must_use]
    pub const fn source_name(&self) -> &str {
        self.source_name
    }

    #[must_use]
    pub fn db_pool<T: 'static>(&self) -> Option<&T> {
        self.db_pool.downcast_ref::<T>()
    }
}

#[async_trait]
pub trait ContentDataProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;

    fn applies_to_sources(&self) -> Vec<String> {
        vec![]
    }

    async fn enrich_content(&self, ctx: &ContentDataContext<'_>, item: &mut Value) -> Result<()>;

    fn priority(&self) -> u32 {
        100
    }
}
