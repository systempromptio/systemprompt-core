//! [`ContentDataProvider`] contract for enriching content items with extra
//! data joined from outside the source file (database lookups, etc.).
//!
//! Providers are held as `Arc<dyn ContentDataProvider>` by the prerender
//! context, so the trait uses `#[async_trait]`; native `async fn` in traits
//! is not `dyn`-compatible.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use serde_json::Value;
use std::any::Any;

use crate::dependencies::{Dependencies, MissingDependency};
use crate::error::ProviderResult;

pub struct ContentDataContext<'a> {
    content_id: &'a str,
    source_name: &'a str,
    dependencies: &'a Dependencies,
}

impl std::fmt::Debug for ContentDataContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentDataContext")
            .field("content_id", &self.content_id)
            .field("source_name", &self.source_name)
            .field("dependencies", self.dependencies)
            .finish()
    }
}

impl<'a> ContentDataContext<'a> {
    #[must_use]
    pub const fn new(
        content_id: &'a str,
        source_name: &'a str,
        dependencies: &'a Dependencies,
    ) -> Self {
        Self {
            content_id,
            source_name,
            dependencies,
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

    pub fn get<T: Any + Send + Sync>(&self) -> Result<&T, MissingDependency> {
        self.dependencies.get::<T>()
    }
}

#[async_trait]
pub trait ContentDataProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;

    fn applies_to_sources(&self) -> Vec<String> {
        vec![]
    }

    async fn enrich_content(
        &self,
        ctx: &ContentDataContext<'_>,
        // JSON: Handlebars template context item; the page data model is dynamic.
        item: &mut Value,
    ) -> ProviderResult<()>;

    fn priority(&self) -> u32 {
        100
    }
}
