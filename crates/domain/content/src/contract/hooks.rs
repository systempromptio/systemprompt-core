//! [`ContentHooks`], the content slot's built value.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_provider_contracts::{
    ComponentRenderer, ContentDataProvider, FrontmatterProcessor, PageDataProvider,
    PagePrerenderer, RssFeedProvider, SitemapProvider, TemplateDataExtender, TemplateProvider,
};

/// The built content hooks of every admitted member, in composition order.
#[derive(Default, Clone)]
pub struct ContentHooks {
    pub(super) template_providers: Vec<Arc<dyn TemplateProvider>>,
    pub(super) component_renderers: Vec<Arc<dyn ComponentRenderer>>,
    pub(super) template_data_extenders: Vec<Arc<dyn TemplateDataExtender>>,
    pub(super) page_data_providers: Vec<Arc<dyn PageDataProvider>>,
    pub(super) page_prerenderers: Vec<Arc<dyn PagePrerenderer>>,
    pub(super) frontmatter_processors: Vec<Arc<dyn FrontmatterProcessor>>,
    pub(super) content_data_providers: Vec<Arc<dyn ContentDataProvider>>,
    pub(super) rss_feed_providers: Vec<Arc<dyn RssFeedProvider>>,
    pub(super) sitemap_providers: Vec<Arc<dyn SitemapProvider>>,
}

impl std::fmt::Debug for ContentHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentHooks")
            .field("template_providers", &self.template_providers.len())
            .field("component_renderers", &self.component_renderers.len())
            .field(
                "template_data_extenders",
                &self.template_data_extenders.len(),
            )
            .field("page_data_providers", &self.page_data_providers.len())
            .field("page_prerenderers", &self.page_prerenderers.len())
            .field("frontmatter_processors", &self.frontmatter_processors.len())
            .field("content_data_providers", &self.content_data_providers.len())
            .field("rss_feed_providers", &self.rss_feed_providers.len())
            .field("sitemap_providers", &self.sitemap_providers.len())
            .finish()
    }
}

impl ContentHooks {
    #[must_use]
    pub fn template_providers(&self) -> &[Arc<dyn TemplateProvider>] {
        &self.template_providers
    }

    #[must_use]
    pub fn component_renderers(&self) -> &[Arc<dyn ComponentRenderer>] {
        &self.component_renderers
    }

    #[must_use]
    pub fn template_data_extenders(&self) -> &[Arc<dyn TemplateDataExtender>] {
        &self.template_data_extenders
    }

    #[must_use]
    pub fn page_data_providers(&self) -> &[Arc<dyn PageDataProvider>] {
        &self.page_data_providers
    }

    #[must_use]
    pub fn page_prerenderers(&self) -> &[Arc<dyn PagePrerenderer>] {
        &self.page_prerenderers
    }

    #[must_use]
    pub fn frontmatter_processors(&self) -> &[Arc<dyn FrontmatterProcessor>] {
        &self.frontmatter_processors
    }

    #[must_use]
    pub fn content_data_providers(&self) -> &[Arc<dyn ContentDataProvider>] {
        &self.content_data_providers
    }

    #[must_use]
    pub fn rss_feed_providers(&self) -> &[Arc<dyn RssFeedProvider>] {
        &self.rss_feed_providers
    }

    #[must_use]
    pub fn sitemap_providers(&self) -> &[Arc<dyn SitemapProvider>] {
        &self.sitemap_providers
    }
}
