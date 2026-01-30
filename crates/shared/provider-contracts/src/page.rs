use std::any::Any;

use crate::web_config::WebConfig;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

pub struct PageContext<'a> {
    pub page_type: &'a str,
    pub web_config: &'a WebConfig,
    content_config: &'a (dyn Any + Send + Sync),
    db_pool: &'a (dyn Any + Send + Sync),
    content_item: Option<&'a Value>,
    all_items: Option<&'a [Value]>,
}

impl std::fmt::Debug for PageContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PageContext")
            .field("page_type", &self.page_type)
            .field("web_config", &"<WebConfig>")
            .field("content_config", &"<dyn Any>")
            .field("db_pool", &"<dyn Any>")
            .field("content_item", &self.content_item.is_some())
            .field("all_items_count", &self.all_items.map(<[_]>::len))
            .finish()
    }
}

impl<'a> PageContext<'a> {
    #[must_use]
    pub fn new(
        page_type: &'a str,
        web_config: &'a WebConfig,
        content_config: &'a (dyn Any + Send + Sync),
        db_pool: &'a (dyn Any + Send + Sync),
    ) -> Self {
        Self {
            page_type,
            web_config,
            content_config,
            db_pool,
            content_item: None,
            all_items: None,
        }
    }

    #[must_use]
    pub const fn with_content_item(mut self, item: &'a Value) -> Self {
        self.content_item = Some(item);
        self
    }

    #[must_use]
    pub const fn with_all_items(mut self, items: &'a [Value]) -> Self {
        self.all_items = Some(items);
        self
    }

    #[must_use]
    pub fn content_config<T: 'static>(&self) -> Option<&T> {
        self.content_config.downcast_ref::<T>()
    }

    #[must_use]
    pub fn db_pool<T: 'static>(&self) -> Option<&T> {
        self.db_pool.downcast_ref::<T>()
    }

    #[must_use]
    pub const fn content_item(&self) -> Option<&Value> {
        self.content_item
    }

    #[must_use]
    pub const fn all_items(&self) -> Option<&[Value]> {
        self.all_items
    }
}

#[async_trait]
pub trait PageDataProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;

    fn applies_to_pages(&self) -> Vec<String> {
        vec![]
    }

    async fn provide_page_data(&self, ctx: &PageContext<'_>) -> Result<Value>;

    fn priority(&self) -> u32 {
        100
    }
}
