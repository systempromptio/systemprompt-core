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
use crate::router::{ExtensionRouterConfig, SiteAuthConfig};
#[cfg(feature = "web")]
use crate::router::ExtensionRouter;

pub trait Extension: Send + Sync + 'static {
    fn metadata(&self) -> ExtensionMetadata;

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![]
    }

    fn migration_weight(&self) -> u32 {
        100
    }

    #[cfg(feature = "web")]
    fn router(&self, ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        let _ = ctx;
        None
    }

    fn router_config(&self) -> Option<ExtensionRouterConfig> {
        None
    }

    fn site_auth(&self) -> Option<SiteAuthConfig> {
        None
    }

    fn jobs(&self) -> Vec<Arc<dyn Job>> {
        vec![]
    }

    fn config_prefix(&self) -> Option<&str> {
        None
    }

    fn config_schema(&self) -> Option<JsonValue> {
        None
    }

    fn validate_config(&self, config: &JsonValue) -> Result<(), ConfigError> {
        let _ = config;
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

    #[cfg(feature = "web")]
    fn has_router(&self, ctx: &dyn ExtensionContext) -> bool {
        self.router(ctx).is_some()
    }

    #[cfg(not(feature = "web"))]
    fn has_router(&self, _ctx: &dyn ExtensionContext) -> bool {
        false
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

#[macro_export]
macro_rules! register_extension {
    ($ext_type:ty) => {
        ::inventory::submit! {
            $crate::ExtensionRegistration {
                factory: || ::std::sync::Arc::new(<$ext_type>::default()) as ::std::sync::Arc<dyn $crate::Extension>,
            }
        }
    };
    ($ext_expr:expr) => {
        ::inventory::submit! {
            $crate::ExtensionRegistration {
                factory: || ::std::sync::Arc::new($ext_expr) as ::std::sync::Arc<dyn $crate::Extension>,
            }
        }
    };
}
