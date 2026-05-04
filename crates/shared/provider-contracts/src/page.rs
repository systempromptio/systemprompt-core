//! [`PageDataProvider`] contract for supplying per-page template data.

use std::any::Any;

use async_trait::async_trait;
use serde_json::Value;

use crate::error::ProviderResult;
use crate::web_config::WebConfig;

/// Per-call context handed to a [`PageDataProvider`].
pub struct PageContext<'a> {
    /// Logical page type, e.g. `blog`, `docs`, `homepage`.
    pub page_type: &'a str,
    /// Resolved web config for the rendering host.
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
    /// Build a [`PageContext`] without per-item or list data attached.
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

    /// Attach the content item being rendered (single-item pages).
    #[must_use]
    pub const fn with_content_item(mut self, item: &'a Value) -> Self {
        self.content_item = Some(item);
        self
    }

    /// Attach the full item list backing the page (list pages).
    #[must_use]
    pub const fn with_all_items(mut self, items: &'a [Value]) -> Self {
        self.all_items = Some(items);
        self
    }

    /// Type-erased downcast of the host's content config.
    #[must_use]
    pub fn content_config<T: 'static>(&self) -> Option<&T> {
        self.content_config.downcast_ref::<T>()
    }

    /// Type-erased downcast of the host's database pool.
    #[must_use]
    pub fn db_pool<T: 'static>(&self) -> Option<&T> {
        self.db_pool.downcast_ref::<T>()
    }

    /// Single content item attached via [`PageContext::with_content_item`].
    #[must_use]
    pub const fn content_item(&self) -> Option<&Value> {
        self.content_item
    }

    /// Item list attached via [`PageContext::with_all_items`].
    #[must_use]
    pub const fn all_items(&self) -> Option<&[Value]> {
        self.all_items
    }
}

/// Hook that supplies template-data variables for a page.
///
/// Marked `#[async_trait]` because it is consumed via `dyn PageDataProvider`.
#[async_trait]
pub trait PageDataProvider: Send + Sync {
    /// Stable identifier for this provider.
    fn provider_id(&self) -> &'static str;

    /// Page-type names this provider opts into; empty means "all".
    fn applies_to_pages(&self) -> Vec<String> {
        vec![]
    }

    /// Compute the JSON variables to merge into the page's template data.
    async fn provide_page_data(&self, ctx: &PageContext<'_>) -> ProviderResult<Value>;

    /// Provider priority; higher runs first.
    fn priority(&self) -> u32 {
        100
    }
}
