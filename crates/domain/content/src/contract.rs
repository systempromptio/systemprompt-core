//! The content capability's contribution point: [`ContentSlot`].
//!
//! A member fills the slot's [`ContentSlotBuilder`] from its `contribute` with
//! `c.slot::<ContentSlot>()`; every builder method only stores the factory it
//! is given. After admission has dropped every disabled member's builder, the
//! runtime builds the slot once: each factory runs against the resolved
//! state, in composition order, and the providers land in [`ContentHooks`],
//! which the generator and the ingestion job read as `Arc<ContentHooks>`.
//!
//! The nine hook kinds are the template providers, component renderers,
//! template data extenders, page data providers, page prerenderers,
//! frontmatter processors, content data providers, and the RSS feed and
//! sitemap providers. Content data providers are ordered by their priority;
//! the template registry orders the rest it holds by theirs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::{Factory, FactoryError, Resolved, Slot, SlotEntry, SlotError, SlotOrder};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{
    ComponentRenderer, ContentDataProvider, FrontmatterProcessor, PageDataProvider,
    PagePrerenderer, RssFeedProvider, SitemapProvider, TemplateDataExtender, TemplateProvider,
};

const SLOT: &str = "content";

/// The content hooks contribution point, built by the runtime into
/// [`ContentHooks`].
#[derive(Debug, Clone, Copy)]
pub struct ContentSlot;

impl Slot for ContentSlot {
    type Builder = ContentSlotBuilder;
    type Built = ContentHooks;
    const ORDER: SlotOrder = SlotOrder::Composition;

    fn build(
        entries: Vec<SlotEntry<Self::Builder>>,
        r: &Resolved<'_>,
    ) -> Result<Self::Built, SlotError> {
        let mut hooks = ContentHooks::default();
        for entry in entries {
            entry.builder.build_into(&entry.owner, r, &mut hooks)?;
        }
        hooks.content_data_providers.sort_by_key(|p| p.priority());
        Ok(hooks)
    }
}

type Factories<T> = Vec<Box<dyn Factory<Arc<T>>>>;

/// One member's content hook factories.
#[derive(Default)]
pub struct ContentSlotBuilder {
    template_providers: Factories<dyn TemplateProvider>,
    component_renderers: Factories<dyn ComponentRenderer>,
    template_data_extenders: Factories<dyn TemplateDataExtender>,
    page_data_providers: Factories<dyn PageDataProvider>,
    page_prerenderers: Factories<dyn PagePrerenderer>,
    frontmatter_processors: Factories<dyn FrontmatterProcessor>,
    content_data_providers: Factories<dyn ContentDataProvider>,
    rss_feed_providers: Factories<dyn RssFeedProvider>,
    sitemap_providers: Factories<dyn SitemapProvider>,
}

impl std::fmt::Debug for ContentSlotBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentSlotBuilder")
            .field("template_providers", &self.template_providers.len())
            .field("component_renderers", &self.component_renderers.len())
            .field("template_data_extenders", &self.template_data_extenders.len())
            .field("page_data_providers", &self.page_data_providers.len())
            .field("page_prerenderers", &self.page_prerenderers.len())
            .field("frontmatter_processors", &self.frontmatter_processors.len())
            .field("content_data_providers", &self.content_data_providers.len())
            .field("rss_feed_providers", &self.rss_feed_providers.len())
            .field("sitemap_providers", &self.sitemap_providers.len())
            .finish()
    }
}

impl ContentSlotBuilder {
    pub fn template_provider(
        &mut self,
        f: impl Factory<Arc<dyn TemplateProvider>>,
    ) -> &mut Self {
        self.template_providers.push(Box::new(f));
        self
    }

    pub fn component_renderer(
        &mut self,
        f: impl Factory<Arc<dyn ComponentRenderer>>,
    ) -> &mut Self {
        self.component_renderers.push(Box::new(f));
        self
    }

    pub fn template_data_extender(
        &mut self,
        f: impl Factory<Arc<dyn TemplateDataExtender>>,
    ) -> &mut Self {
        self.template_data_extenders.push(Box::new(f));
        self
    }

    pub fn page_data_provider(
        &mut self,
        f: impl Factory<Arc<dyn PageDataProvider>>,
    ) -> &mut Self {
        self.page_data_providers.push(Box::new(f));
        self
    }

    pub fn page_prerenderer(&mut self, f: impl Factory<Arc<dyn PagePrerenderer>>) -> &mut Self {
        self.page_prerenderers.push(Box::new(f));
        self
    }

    pub fn frontmatter_processor(
        &mut self,
        f: impl Factory<Arc<dyn FrontmatterProcessor>>,
    ) -> &mut Self {
        self.frontmatter_processors.push(Box::new(f));
        self
    }

    pub fn content_data_provider(
        &mut self,
        f: impl Factory<Arc<dyn ContentDataProvider>>,
    ) -> &mut Self {
        self.content_data_providers.push(Box::new(f));
        self
    }

    pub fn rss_feed_provider(&mut self, f: impl Factory<Arc<dyn RssFeedProvider>>) -> &mut Self {
        self.rss_feed_providers.push(Box::new(f));
        self
    }

    pub fn sitemap_provider(&mut self, f: impl Factory<Arc<dyn SitemapProvider>>) -> &mut Self {
        self.sitemap_providers.push(Box::new(f));
        self
    }

    fn build_into(
        self,
        owner: &ExtensionId,
        r: &Resolved<'_>,
        hooks: &mut ContentHooks,
    ) -> Result<(), SlotError> {
        run(owner, r, self.template_providers, &mut hooks.template_providers)?;
        run(owner, r, self.component_renderers, &mut hooks.component_renderers)?;
        run(
            owner,
            r,
            self.template_data_extenders,
            &mut hooks.template_data_extenders,
        )?;
        run(owner, r, self.page_data_providers, &mut hooks.page_data_providers)?;
        run(owner, r, self.page_prerenderers, &mut hooks.page_prerenderers)?;
        run(
            owner,
            r,
            self.frontmatter_processors,
            &mut hooks.frontmatter_processors,
        )?;
        run(
            owner,
            r,
            self.content_data_providers,
            &mut hooks.content_data_providers,
        )?;
        run(owner, r, self.rss_feed_providers, &mut hooks.rss_feed_providers)?;
        run(owner, r, self.sitemap_providers, &mut hooks.sitemap_providers)
    }
}

fn run<T: ?Sized + 'static>(
    owner: &ExtensionId,
    r: &Resolved<'_>,
    factories: Factories<T>,
    out: &mut Vec<Arc<T>>,
) -> Result<(), SlotError> {
    for factory in factories {
        out.push(factory.build(r).map_err(|error| match error {
            FactoryError::Missing(missing) => SlotError::Missing(missing),
            FactoryError::Failed(source) => SlotError::Entry {
                slot: SLOT,
                owner: owner.clone(),
                source,
            },
        })?);
    }
    Ok(())
}

/// The built content hooks of every admitted member, in composition order.
#[derive(Default, Clone)]
pub struct ContentHooks {
    template_providers: Vec<Arc<dyn TemplateProvider>>,
    component_renderers: Vec<Arc<dyn ComponentRenderer>>,
    template_data_extenders: Vec<Arc<dyn TemplateDataExtender>>,
    page_data_providers: Vec<Arc<dyn PageDataProvider>>,
    page_prerenderers: Vec<Arc<dyn PagePrerenderer>>,
    frontmatter_processors: Vec<Arc<dyn FrontmatterProcessor>>,
    content_data_providers: Vec<Arc<dyn ContentDataProvider>>,
    rss_feed_providers: Vec<Arc<dyn RssFeedProvider>>,
    sitemap_providers: Vec<Arc<dyn SitemapProvider>>,
}

impl std::fmt::Debug for ContentHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentHooks")
            .field("template_providers", &self.template_providers.len())
            .field("component_renderers", &self.component_renderers.len())
            .field("template_data_extenders", &self.template_data_extenders.len())
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
