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
pub use router::{ExtensionRouter, ExtensionRouterConfig, SiteAuthConfig};
pub use traits::Extension;

pub use any::{AnyExtension, ApiExtensionWrapper, ExtensionWrapper, SchemaExtensionWrapper};
pub use builder::ExtensionBuilder;
pub use capabilities::{
    CapabilityContext, FullContext, HasAnalytics, HasConfig, HasDatabase, HasEventBus,
    HasExtension, HasFingerprint, HasHttpClient, HasRouteClassifier, HasUserService,
};
pub use hlist::{Contains, NotSame, Subset, TypeList};
pub use typed::{
    ApiExtensionTyped, ApiExtensionTypedDyn, ConfigExtensionTyped, JobExtensionTyped,
    ProviderExtensionTyped, SchemaDefinitionTyped, SchemaExtensionTyped, SchemaSourceTyped,
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
        Extension, ExtensionMetadata, ExtensionRole, ExtensionRouter, Migration, SchemaDefinition,
        SchemaSource, SiteAuthConfig, register_extension,
    };

    pub use crate::any::AnyExtension;
    pub use crate::builder::ExtensionBuilder;
    pub use crate::capabilities::{
        CapabilityContext, FullContext, HasConfig, HasDatabase, HasEventBus, HasExtension,
        HasHttpClient,
    };

    pub use crate::hlist::{Contains, NotSame, Subset, TypeList};
    pub use crate::typed::{
        ApiExtensionTyped, ApiExtensionTypedDyn, ConfigExtensionTyped, JobExtensionTyped,
        ProviderExtensionTyped, SchemaDefinitionTyped, SchemaExtensionTyped, SchemaSourceTyped,
    };
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
