use std::any::Any;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

pub struct ExtenderContext<'a> {
    pub item: &'a Value,
    pub all_items: &'a [Value],
    pub config: &'a serde_yaml::Value,
    pub web_config: &'a serde_yaml::Value,
    pub content_html: &'a str,
    pub url_pattern: &'a str,
    pub source_name: &'a str,
    db_pool: &'a (dyn Any + Send + Sync),
}

impl std::fmt::Debug for ExtenderContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtenderContext")
            .field("item", &self.item)
            .field("all_items", &format!("[{} items]", self.all_items.len()))
            .field(
                "content_html",
                &format!("[{} chars]", self.content_html.len()),
            )
            .field("url_pattern", &self.url_pattern)
            .field("source_name", &self.source_name)
            .field("db_pool", &"<dyn Any>")
            .finish()
    }
}

impl<'a> ExtenderContext<'a> {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        item: &'a Value,
        all_items: &'a [Value],
        config: &'a serde_yaml::Value,
        web_config: &'a serde_yaml::Value,
        content_html: &'a str,
        url_pattern: &'a str,
        source_name: &'a str,
        db_pool: &'a (dyn Any + Send + Sync),
    ) -> Self {
        Self {
            item,
            all_items,
            config,
            web_config,
            content_html,
            url_pattern,
            source_name,
            db_pool,
        }
    }

    #[must_use]
    pub fn db_pool<T: 'static>(&self) -> Option<&T> {
        self.db_pool.downcast_ref::<T>()
    }
}

#[derive(Debug)]
pub struct ExtendedData {
    pub variables: Value,
    pub priority: u32,
}

impl ExtendedData {
    #[must_use]
    pub const fn new(variables: Value) -> Self {
        Self {
            variables,
            priority: 100,
        }
    }

    #[must_use]
    pub const fn with_priority(variables: Value, priority: u32) -> Self {
        Self {
            variables,
            priority,
        }
    }
}

#[async_trait]
pub trait TemplateDataExtender: Send + Sync {
    fn extender_id(&self) -> &str;

    fn applies_to(&self) -> Vec<String> {
        vec![]
    }

    async fn extend(&self, ctx: &ExtenderContext<'_>, data: &mut Value) -> Result<()>;

    fn priority(&self) -> u32 {
        100
    }
}
