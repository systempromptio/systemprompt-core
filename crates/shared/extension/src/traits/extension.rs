//! The core `Extension` trait defining metadata, schemas, routers, and
//! providers an extension contributes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::Value as JsonValue;
use systemprompt_provider_contracts::{
    ComponentRenderer, ContentDataProvider, FrontmatterProcessor, Job, LlmProvider,
    PageDataProvider, PagePrerenderer, RssFeedProvider, SitemapProvider, TemplateDataExtender,
    TemplateProvider, ToolProvider,
};

use crate::asset::{AssetDefinition, AssetPaths};
use crate::context::ExtensionContext;
use crate::error::ConfigError;
use crate::metadata::{ExtensionMetadata, ExtensionRole, SchemaDefinition};
use crate::migration::Migration;
use crate::router::{ExtensionRouter, ExtensionRouterConfig, SiteAuthConfig};
use crate::seed::Seed;

pub trait Extension: Send + Sync + 'static {
    fn metadata(&self) -> ExtensionMetadata;

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![]
    }

    fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        None
    }

    fn router_config(&self) -> Option<ExtensionRouterConfig> {
        None
    }

    fn site_auth(&self) -> Option<SiteAuthConfig> {
        None
    }

    /// Per-extension job manifest, for CLI/plugin attribution only.
    ///
    /// The scheduler does **not** consult this method: it discovers runnable
    /// jobs from the `inventory` catalog populated by `submit_job!`. This list
    /// is used by `jobs list` and plugin-capability commands to attribute a job
    /// to the extension that owns it; an entry here that is never
    /// `submit_job!`d is invisible to scheduling.
    fn jobs(&self) -> Vec<Arc<dyn Job>> {
        vec![]
    }

    fn config_prefix(&self) -> Option<&str> {
        None
    }

    fn config_schema(&self) -> Option<JsonValue> {
        None
    }

    fn validate_config(&self, _config: &JsonValue) -> Result<(), ConfigError> {
        Ok(())
    }

    fn llm_providers(&self) -> Vec<Arc<dyn LlmProvider>> {
        vec![]
    }

    fn tool_providers(&self) -> Vec<Arc<dyn ToolProvider>> {
        vec![]
    }

    fn template_providers(&self) -> Vec<Arc<dyn TemplateProvider>> {
        vec![]
    }

    fn component_renderers(&self) -> Vec<Arc<dyn ComponentRenderer>> {
        vec![]
    }

    fn template_data_extenders(&self) -> Vec<Arc<dyn TemplateDataExtender>> {
        vec![]
    }

    fn page_data_providers(&self) -> Vec<Arc<dyn PageDataProvider>> {
        vec![]
    }

    fn page_prerenderers(&self) -> Vec<Arc<dyn PagePrerenderer>> {
        vec![]
    }

    fn frontmatter_processors(&self) -> Vec<Arc<dyn FrontmatterProcessor>> {
        vec![]
    }

    fn content_data_providers(&self) -> Vec<Arc<dyn ContentDataProvider>> {
        vec![]
    }

    fn rss_feed_providers(&self) -> Vec<Arc<dyn RssFeedProvider>> {
        vec![]
    }

    fn sitemap_providers(&self) -> Vec<Arc<dyn SitemapProvider>> {
        vec![]
    }

    fn required_storage_paths(&self) -> Vec<&'static str> {
        vec![]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }

    fn is_required(&self) -> bool {
        false
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![]
    }

    fn seeds(&self) -> Vec<Seed> {
        Vec::new()
    }

    /// Tables this extension is permitted to mutate with a cross-extension
    /// `ALTER` even though another extension creates them. The tables an
    /// extension *owns* are derived from the `CREATE TABLE` statements in its
    /// [`Extension::schemas`] and must not be repeated here.
    fn cross_extension_tables(&self) -> Vec<&'static str> {
        Vec::new()
    }

    fn roles(&self) -> Vec<ExtensionRole> {
        vec![]
    }

    fn priority(&self) -> u32 {
        100
    }

    fn id(&self) -> &'static str {
        self.metadata().id
    }

    fn name(&self) -> &'static str {
        self.metadata().name
    }

    fn version(&self) -> &'static str {
        self.metadata().version
    }

    fn has_schemas(&self) -> bool {
        !self.schemas().is_empty()
    }

    fn has_router(&self, ctx: &dyn ExtensionContext) -> bool {
        self.router(ctx).is_some()
    }

    fn has_jobs(&self) -> bool {
        !self.jobs().is_empty()
    }

    fn has_config(&self) -> bool {
        self.config_prefix().is_some()
    }

    fn has_llm_providers(&self) -> bool {
        !self.llm_providers().is_empty()
    }

    fn has_tool_providers(&self) -> bool {
        !self.tool_providers().is_empty()
    }

    fn has_template_providers(&self) -> bool {
        !self.template_providers().is_empty()
    }

    fn has_component_renderers(&self) -> bool {
        !self.component_renderers().is_empty()
    }

    fn has_template_data_extenders(&self) -> bool {
        !self.template_data_extenders().is_empty()
    }

    fn has_page_data_providers(&self) -> bool {
        !self.page_data_providers().is_empty()
    }

    fn has_page_prerenderers(&self) -> bool {
        !self.page_prerenderers().is_empty()
    }

    fn has_frontmatter_processors(&self) -> bool {
        !self.frontmatter_processors().is_empty()
    }

    fn has_content_data_providers(&self) -> bool {
        !self.content_data_providers().is_empty()
    }

    fn has_rss_feed_providers(&self) -> bool {
        !self.rss_feed_providers().is_empty()
    }

    fn has_sitemap_providers(&self) -> bool {
        !self.sitemap_providers().is_empty()
    }

    fn has_site_auth(&self) -> bool {
        self.site_auth().is_some()
    }

    fn has_storage_paths(&self) -> bool {
        !self.required_storage_paths().is_empty()
    }

    fn has_roles(&self) -> bool {
        !self.roles().is_empty()
    }

    fn has_migrations(&self) -> bool {
        !self.migrations().is_empty()
    }

    fn declares_assets(&self) -> bool {
        false
    }

    fn required_assets(&self, _paths: &dyn AssetPaths) -> Vec<AssetDefinition> {
        vec![]
    }
}
