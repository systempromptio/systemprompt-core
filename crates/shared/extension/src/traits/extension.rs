//! The core `Extension` trait defining metadata, schemas, routers, and
//! providers an extension contributes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::Value as JsonValue;
use systemprompt_provider_contracts::{
    ComponentRenderer, ConfigPlane, ContentDataProvider, FrontmatterProcessor, PageDataProvider,
    PagePrerenderer, RssFeedProvider, SitemapProvider, TemplateDataExtender, TemplateProvider,
    ToolProvider,
};
use systemprompt_traits::RunnerService;

use crate::asset::{AssetDefinition, AssetPaths};
use crate::context::ExtensionContext;
use crate::error::{ExtensionConfigError, ExtensionStateError};
use crate::health::{MetricDescriptor, ReadinessCheck};
use crate::metadata::{ExtensionMetadata, ExtensionRole, SchemaDefinition};
use crate::migration::Migration;
use crate::router::{ExtensionRouter, SiteAuthConfig};
use crate::seed::Seed;
use crate::state::{StateBuilder, StateRequirement};

/// The one contract an extension implements: `metadata()` is required and
/// every other method is a defaulted hook.
///
/// `provide` runs once at boot, synchronously and in dependency order, for
/// every enabled extension, and contributes typed values to the shared
/// extension state ([`state`](crate::state)); `requires` names the types the
/// extension cannot run without, checked before anything serves.
pub trait Extension: Send + Sync + 'static {
    fn metadata(&self) -> ExtensionMetadata;

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![]
    }

    fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        None
    }

    fn site_auth(&self) -> Option<SiteAuthConfig> {
        None
    }

    fn config_planes(&self) -> Vec<Arc<dyn ConfigPlane>> {
        vec![]
    }

    fn runner_services(&self) -> Vec<Arc<dyn RunnerService>> {
        vec![]
    }

    fn readiness(&self, _ctx: &dyn ExtensionContext) -> Vec<Arc<dyn ReadinessCheck>> {
        vec![]
    }

    fn metrics(&self) -> Vec<MetricDescriptor> {
        vec![]
    }

    fn config_prefix(&self) -> Option<&str> {
        None
    }

    // JSON: Extension config block from the profile YAML; the extension owns it.
    fn config_schema(&self) -> Option<JsonValue> {
        None
    }

    // JSON: Extension config block from the profile YAML; the extension owns it.
    fn validate_config(&self, _config: &JsonValue) -> Result<(), ExtensionConfigError> {
        Ok(())
    }

    fn tool_providers(&self) -> Vec<Arc<dyn ToolProvider>> {
        vec![]
    }

    fn template_providers(&self) -> Vec<Arc<dyn TemplateProvider>> {
        vec![]
    }

    fn component_renderers(&self, _ctx: &dyn ExtensionContext) -> Vec<Arc<dyn ComponentRenderer>> {
        vec![]
    }

    fn template_data_extenders(
        &self,
        _ctx: &dyn ExtensionContext,
    ) -> Vec<Arc<dyn TemplateDataExtender>> {
        vec![]
    }

    fn page_data_providers(&self, _ctx: &dyn ExtensionContext) -> Vec<Arc<dyn PageDataProvider>> {
        vec![]
    }

    fn page_prerenderers(&self, _ctx: &dyn ExtensionContext) -> Vec<Arc<dyn PagePrerenderer>> {
        vec![]
    }

    fn frontmatter_processors(&self) -> Vec<Arc<dyn FrontmatterProcessor>> {
        vec![]
    }

    fn content_data_providers(
        &self,
        _ctx: &dyn ExtensionContext,
    ) -> Vec<Arc<dyn ContentDataProvider>> {
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

    fn provide(
        &self,
        _ctx: &dyn ExtensionContext,
        _state: &mut StateBuilder<'_>,
    ) -> Result<(), ExtensionStateError> {
        Ok(())
    }

    fn requires(&self) -> Vec<StateRequirement> {
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

    fn retirements(&self) -> Vec<SchemaDefinition> {
        Vec::new()
    }

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

    fn has_config(&self) -> bool {
        self.config_prefix().is_some()
    }

    fn has_tool_providers(&self) -> bool {
        !self.tool_providers().is_empty()
    }

    fn has_template_providers(&self) -> bool {
        !self.template_providers().is_empty()
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
