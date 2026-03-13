pub mod any;
mod asset;
pub mod builder;
pub mod capabilities;
pub mod context;
pub mod error;
pub mod hlist;
pub mod metadata;
pub mod migration;
pub mod registry;
pub mod router;
pub mod runtime_config;
mod traits;
pub mod typed;
pub mod typed_registry;
pub mod types;

pub use asset::{AssetDefinition, AssetDefinitionBuilder, AssetPaths, AssetType};
pub use context::{DynExtensionContext, ExtensionContext};
pub use error::{ConfigError, LoaderError};
pub use metadata::{ExtensionMetadata, ExtensionRole, SchemaDefinition, SchemaSource, SeedSource};
pub use migration::Migration;
pub use registry::{ExtensionRegistration, ExtensionRegistry};
pub use router::{ExtensionRouterConfig, SiteAuthConfig};
pub use traits::Extension;
#[cfg(feature = "web")]
pub use router::ExtensionRouter;

#[cfg(feature = "web")]
pub use any::ApiExtensionWrapper;
pub use any::{AnyExtension, ExtensionWrapper, SchemaExtensionWrapper};
pub use builder::ExtensionBuilder;
#[cfg(feature = "web")]
pub use capabilities::HasHttpClient;
pub use capabilities::{
    CapabilityContext, FullContext, HasConfig, HasDatabase, HasEventBus, HasExtension,
};
pub use hlist::{Contains, NotSame, Subset, TypeList};
#[cfg(feature = "web")]
pub use typed::ApiExtensionTypedDyn;
pub use typed::{
    ApiExtensionTyped, ConfigExtensionTyped, JobExtensionTyped, ProviderExtensionTyped,
    SchemaDefinitionTyped, SchemaExtensionTyped, SchemaSourceTyped,
};
pub use typed_registry::{RESERVED_PATHS, TypedExtensionRegistry};
pub use types::{
    Dependencies, DependencyList, ExtensionMeta, ExtensionType, MissingDependency, NoDependencies,
};

pub mod prelude {
    pub use crate::asset::{AssetDefinition, AssetDefinitionBuilder, AssetPaths, AssetType};
    pub use crate::context::{DynExtensionContext, ExtensionContext};
    pub use crate::error::{ConfigError, LoaderError};
    pub use crate::registry::ExtensionRegistry;
    pub use crate::{
        Extension, ExtensionMetadata, ExtensionRole, Migration, SchemaDefinition, SchemaSource,
        SiteAuthConfig, register_extension,
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
    pub use crate::typed_registry::{RESERVED_PATHS, TypedExtensionRegistry};
    pub use crate::types::{
        Dependencies, DependencyList, ExtensionMeta, ExtensionType, MissingDependency,
        NoDependencies,
    };

    pub use systemprompt_provider_contracts::{
        ComponentContext, ComponentRenderer, ContentDataContext, ContentDataProvider,
        FrontmatterContext, FrontmatterProcessor, PageContext, PageDataProvider,
        PagePrepareContext, PagePrerenderer, PageRenderSpec, PlaceholderMapping, RenderedComponent,
        RssFeedContext, RssFeedItem, RssFeedMetadata, RssFeedProvider, RssFeedSpec, SitemapContext,
        SitemapProvider, SitemapSourceSpec, SitemapUrlEntry, TemplateDataExtender,
        TemplateDefinition, TemplateProvider, TemplateSource,
    };
}
