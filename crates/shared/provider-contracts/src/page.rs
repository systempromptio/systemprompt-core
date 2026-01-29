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
}

impl std::fmt::Debug for PageContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PageContext")
            .field("page_type", &self.page_type)
            .field("web_config", &"<WebConfig>")
            .field("content_config", &"<dyn Any>")
            .field("db_pool", &"<dyn Any>")
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
        }
    }

    #[must_use]
    pub fn content_config<T: 'static>(&self) -> Option<&T> {
        self.content_config.downcast_ref::<T>()
    }

    #[must_use]
    pub fn db_pool<T: 'static>(&self) -> Option<&T> {
        self.db_pool.downcast_ref::<T>()
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
