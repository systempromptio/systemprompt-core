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

/// Type-erased extension contract.
///
/// `Extension` is the dyn-compatible bridge between `inventory`-registered
/// extensions and the runtime registry. Use the typed sub-traits in
/// [`crate::typed`] for compile-time-checked composition; implement
/// `Extension` directly only for top-level registrations that need to be
/// stored as `Arc<dyn Extension>`.
pub trait Extension: Send + Sync + 'static {
    /// Returns the extension's static metadata block (id, name, version).
    fn metadata(&self) -> ExtensionMetadata;

    /// Returns schema definitions contributed to the migration plan.
    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![]
    }

    /// Returns the migration ordering weight (lower runs first).
    fn migration_weight(&self) -> u32 {
        100
    }

    /// Returns the axum router this extension contributes, if any.
    fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        None
    }

    /// Returns the router-mounting configuration this extension expects.
    fn router_config(&self) -> Option<ExtensionRouterConfig> {
        None
    }

    /// Returns the site-level authentication configuration this extension
    /// declares.
    fn site_auth(&self) -> Option<SiteAuthConfig> {
        None
    }

    /// Returns the scheduler jobs this extension registers.
    fn jobs(&self) -> Vec<Arc<dyn Job>> {
        vec![]
    }

    /// Returns the configuration namespace prefix this extension owns.
    fn config_prefix(&self) -> Option<&str> {
        None
    }

    /// Returns a JSON schema describing this extension's configuration.
    fn config_schema(&self) -> Option<JsonValue> {
        None
    }

    /// Validates a runtime configuration block against this extension's
    /// expectations. Defaults to accepting anything.
    fn validate_config(&self, _config: &JsonValue) -> Result<(), ConfigError> {
        Ok(())
    }

    /// Returns the LLM providers this extension contributes.
    fn llm_providers(&self) -> Vec<Arc<dyn LlmProvider>> {
        vec![]
    }

    /// Returns the tool providers this extension contributes.
    fn tool_providers(&self) -> Vec<Arc<dyn ToolProvider>> {
        vec![]
    }

    /// Returns the template providers this extension contributes.
    fn template_providers(&self) -> Vec<Arc<dyn TemplateProvider>> {
        vec![]
    }

    /// Returns the component renderers this extension contributes.
    fn component_renderers(&self) -> Vec<Arc<dyn ComponentRenderer>> {
        vec![]
    }

    /// Returns the template-data extenders this extension contributes.
    fn template_data_extenders(&self) -> Vec<Arc<dyn TemplateDataExtender>> {
        vec![]
    }

    /// Returns the page-data providers this extension contributes.
    fn page_data_providers(&self) -> Vec<Arc<dyn PageDataProvider>> {
        vec![]
    }

    /// Returns the page prerenderers this extension contributes.
    fn page_prerenderers(&self) -> Vec<Arc<dyn PagePrerenderer>> {
        vec![]
    }

    /// Returns the frontmatter processors this extension contributes.
    fn frontmatter_processors(&self) -> Vec<Arc<dyn FrontmatterProcessor>> {
        vec![]
    }

    /// Returns the content-data providers this extension contributes.
    fn content_data_providers(&self) -> Vec<Arc<dyn ContentDataProvider>> {
        vec![]
    }

    /// Returns the RSS feed providers this extension contributes.
    fn rss_feed_providers(&self) -> Vec<Arc<dyn RssFeedProvider>> {
        vec![]
    }

    /// Returns the sitemap providers this extension contributes.
    fn sitemap_providers(&self) -> Vec<Arc<dyn SitemapProvider>> {
        vec![]
    }

    /// Returns subdirectory paths under the storage root this extension
    /// requires to exist.
    fn required_storage_paths(&self) -> Vec<&'static str> {
        vec![]
    }

    /// Returns the IDs of extensions this extension depends on.
    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }

    /// Returns true if this extension cannot be disabled.
    fn is_required(&self) -> bool {
        false
    }

    /// Returns the schema migrations this extension applies.
    fn migrations(&self) -> Vec<Migration> {
        vec![]
    }

    /// Returns the role definitions this extension publishes.
    fn roles(&self) -> Vec<ExtensionRole> {
        vec![]
    }

    /// Returns the priority used to order extensions (lower runs first).
    fn priority(&self) -> u32 {
        100
    }

    /// Returns the extension's stable identifier (`metadata().id`).
    fn id(&self) -> &'static str {
        self.metadata().id
    }

    /// Returns the extension's human-readable name (`metadata().name`).
    fn name(&self) -> &'static str {
        self.metadata().name
    }

    /// Returns the extension's version string (`metadata().version`).
    fn version(&self) -> &'static str {
        self.metadata().version
    }

    /// Returns true if the extension contributes any schema definitions.
    fn has_schemas(&self) -> bool {
        !self.schemas().is_empty()
    }

    /// Returns true if the extension contributes a router for `ctx`.
    fn has_router(&self, ctx: &dyn ExtensionContext) -> bool {
        self.router(ctx).is_some()
    }

    /// Returns true if the extension registers any jobs.
    fn has_jobs(&self) -> bool {
        !self.jobs().is_empty()
    }

    /// Returns true if the extension declares a configuration namespace.
    fn has_config(&self) -> bool {
        self.config_prefix().is_some()
    }

    /// Returns true if the extension contributes any LLM providers.
    fn has_llm_providers(&self) -> bool {
        !self.llm_providers().is_empty()
    }

    /// Returns true if the extension contributes any tool providers.
    fn has_tool_providers(&self) -> bool {
        !self.tool_providers().is_empty()
    }

    /// Returns true if the extension contributes any template providers.
    fn has_template_providers(&self) -> bool {
        !self.template_providers().is_empty()
    }

    /// Returns true if the extension contributes any component renderers.
    fn has_component_renderers(&self) -> bool {
        !self.component_renderers().is_empty()
    }

    /// Returns true if the extension contributes any template-data extenders.
    fn has_template_data_extenders(&self) -> bool {
        !self.template_data_extenders().is_empty()
    }

    /// Returns true if the extension contributes any page-data providers.
    fn has_page_data_providers(&self) -> bool {
        !self.page_data_providers().is_empty()
    }

    /// Returns true if the extension contributes any page prerenderers.
    fn has_page_prerenderers(&self) -> bool {
        !self.page_prerenderers().is_empty()
    }

    /// Returns true if the extension contributes any frontmatter processors.
    fn has_frontmatter_processors(&self) -> bool {
        !self.frontmatter_processors().is_empty()
    }

    /// Returns true if the extension contributes any content-data providers.
    fn has_content_data_providers(&self) -> bool {
        !self.content_data_providers().is_empty()
    }

    /// Returns true if the extension contributes any RSS feed providers.
    fn has_rss_feed_providers(&self) -> bool {
        !self.rss_feed_providers().is_empty()
    }

    /// Returns true if the extension contributes any sitemap providers.
    fn has_sitemap_providers(&self) -> bool {
        !self.sitemap_providers().is_empty()
    }

    /// Returns true if the extension declares site-auth configuration.
    fn has_site_auth(&self) -> bool {
        self.site_auth().is_some()
    }

    /// Returns true if the extension requires storage paths to exist.
    fn has_storage_paths(&self) -> bool {
        !self.required_storage_paths().is_empty()
    }

    /// Returns true if the extension declares any roles.
    fn has_roles(&self) -> bool {
        !self.roles().is_empty()
    }

    /// Returns true if the extension declares any migrations.
    fn has_migrations(&self) -> bool {
        !self.migrations().is_empty()
    }

    /// Returns true if the extension declares any static assets.
    fn declares_assets(&self) -> bool {
        false
    }

    /// Returns the static asset definitions this extension requires given
    /// the resolved asset paths.
    fn required_assets(&self, _paths: &dyn AssetPaths) -> Vec<AssetDefinition> {
        vec![]
    }
}
