pub mod any;
mod asset;
pub mod builder;
pub mod capabilities;
pub mod context;
pub mod error;
pub mod hlist;
pub mod registry;
pub mod runtime_config;
pub mod typed;
pub mod typed_registry;
pub mod types;

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use systemprompt_provider_contracts::{
    ComponentRenderer, Job, LlmProvider, PageDataProvider, TemplateDataExtender, TemplateProvider,
    ToolProvider,
};

pub use asset::{AssetDefinition, AssetDefinitionBuilder, AssetType};
pub use context::{DynExtensionContext, ExtensionContext};
pub use error::{ConfigError, LoaderError};
pub use registry::{ExtensionRegistration, ExtensionRegistry};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ExtensionMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinition {
    pub table: String,
    pub sql: SchemaSource,
    pub required_columns: Vec<String>,
}

impl SchemaDefinition {
    #[must_use]
    pub fn inline(table: impl Into<String>, sql: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            sql: SchemaSource::Inline(sql.into()),
            required_columns: Vec::new(),
        }
    }

    #[must_use]
    pub fn file(table: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            table: table.into(),
            sql: SchemaSource::File(path.into()),
            required_columns: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_required_columns(mut self, columns: Vec<String>) -> Self {
        self.required_columns = columns;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchemaSource {
    Inline(String),
    File(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SeedSource {
    Inline(String),
    File(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionRole {
    pub name: String,
    pub display_name: String,
    pub description: String,
    #[serde(default)]
    pub permissions: Vec<String>,
}

impl ExtensionRole {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        display_name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.into(),
            description: description.into(),
            permissions: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
        self.permissions = permissions;
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ExtensionRouterConfig {
    pub base_path: &'static str,
    pub requires_auth: bool,
}

impl ExtensionRouterConfig {
    #[must_use]
    pub const fn new(base_path: &'static str) -> Self {
        Self {
            base_path,
            requires_auth: true,
        }
    }

    #[must_use]
    pub const fn public(base_path: &'static str) -> Self {
        Self {
            base_path,
            requires_auth: false,
        }
    }
}

#[cfg(feature = "web")]
#[derive(Debug, Clone)]
pub struct ExtensionRouter {
    pub router: axum::Router,
    pub base_path: &'static str,
    pub requires_auth: bool,
}

#[cfg(feature = "web")]
impl ExtensionRouter {
    #[must_use]
    pub const fn new(router: axum::Router, base_path: &'static str) -> Self {
        Self {
            router,
            base_path,
            requires_auth: true,
        }
    }

    #[must_use]
    pub const fn public(router: axum::Router, base_path: &'static str) -> Self {
        Self {
            router,
            base_path,
            requires_auth: false,
        }
    }

    #[must_use]
    pub const fn config(&self) -> ExtensionRouterConfig {
        ExtensionRouterConfig {
            base_path: self.base_path,
            requires_auth: self.requires_auth,
        }
    }
}

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

    fn required_storage_paths(&self) -> Vec<&'static str> {
        vec![]
    }

    fn dependencies(&self) -> Vec<&'static str> {
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

    fn has_storage_paths(&self) -> bool {
        !self.required_storage_paths().is_empty()
    }

    fn has_roles(&self) -> bool {
        !self.roles().is_empty()
    }

    fn required_assets(&self) -> Vec<AssetDefinition> {
        vec![]
    }

    fn has_assets(&self) -> bool {
        !self.required_assets().is_empty()
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

pub mod prelude {
    pub use crate::asset::{AssetDefinition, AssetDefinitionBuilder, AssetType};
    pub use crate::context::{DynExtensionContext, ExtensionContext};
    pub use crate::error::{ConfigError, LoaderError};
    pub use crate::registry::ExtensionRegistry;
    pub use crate::{
        register_extension, Extension, ExtensionMetadata, ExtensionRole, SchemaDefinition,
        SchemaSource,
    };

    #[cfg(feature = "web")]
    pub use crate::ExtensionRouter;

    pub use crate::any::AnyExtension;
    pub use crate::builder::ExtensionBuilder;
    pub use crate::capabilities::{
        CapabilityContext, FullContext, HasConfig, HasDatabase, HasEventBus, HasExtension,
    };

    #[cfg(feature = "web")]
    pub use crate::capabilities::HasHttpClient;
    pub use crate::hlist::{Contains, NotSame, Subset, TypeList};
    pub use crate::typed::{
        ApiExtensionTyped, ConfigExtensionTyped, JobExtensionTyped, ProviderExtensionTyped,
        SchemaDefinitionTyped, SchemaExtensionTyped, SchemaSourceTyped,
    };

    #[cfg(feature = "web")]
    pub use crate::typed::ApiExtensionTypedDyn;
    pub use crate::typed_registry::{TypedExtensionRegistry, RESERVED_PATHS};
    pub use crate::types::{
        Dependencies, DependencyList, ExtensionMeta, ExtensionType, MissingDependency,
        NoDependencies,
    };

    pub use systemprompt_provider_contracts::{
        ComponentContext, ComponentRenderer, PageContext, PageDataProvider, RenderedComponent,
        TemplateDataExtender, TemplateDefinition, TemplateProvider, TemplateSource,
    };
}

pub use any::{AnyExtension, ExtensionWrapper, SchemaExtensionWrapper};
#[cfg(feature = "web")]
pub use any::ApiExtensionWrapper;
pub use builder::ExtensionBuilder;
pub use capabilities::{
    CapabilityContext, FullContext, HasConfig, HasDatabase, HasEventBus, HasExtension,
};
#[cfg(feature = "web")]
pub use capabilities::HasHttpClient;
pub use hlist::{Contains, NotSame, Subset, TypeList};
pub use typed::{
    ApiExtensionTyped, ConfigExtensionTyped, JobExtensionTyped, ProviderExtensionTyped,
    SchemaDefinitionTyped, SchemaExtensionTyped, SchemaSourceTyped,
};
#[cfg(feature = "web")]
pub use typed::ApiExtensionTypedDyn;
pub use typed_registry::{TypedExtensionRegistry, RESERVED_PATHS};
pub use types::{
    Dependencies, DependencyList, ExtensionMeta, ExtensionType, MissingDependency, NoDependencies,
};
