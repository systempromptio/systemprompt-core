//! Compile-time extension framework for systemprompt.io.
//!
//! Built on the [`inventory`] crate, this crate lets extension authors
//! declare schemas, API routes, scheduled jobs, and provider
//! implementations using the [`Extension`] trait and the
//! [`register_extension!`] macro. The runtime collects every registration
//! at startup, validates dependencies, and merges the resulting wiring
//! into the host application.
//!
//! # Authoring an extension
//!
//! ```ignore
//! use systemprompt_extension::prelude::*;
//!
//! #[derive(Default)]
//! struct MyExtension;
//!
//! impl Extension for MyExtension {
//!     fn metadata(&self) -> ExtensionMetadata {
//!         ExtensionMetadata { id: "my-ext", name: "My Extension", version: "0.1.0" }
//!     }
//! }
//!
//! register_extension!(MyExtension);
//! ```
//!
//! # Module map
//!
//! - [`metadata`], [`router`], [`migration`] — value types describing the
//!   extension surface.
//! - The `Extension` trait and the [`register_extension!`] macro form the core
//!   extensibility contract.
//! - [`typed`] — compile-time-checked sub-traits (`SchemaExtensionTyped`,
//!   `ApiExtensionTyped`, ...).
//! - [`registry`] / [`typed_registry`] — runtime stores of registered
//!   extensions.
//! - [`builder`] — fluent builder that enforces dependency ordering at compile
//!   time.
//! - [`capabilities`] — `Has*` capability traits and the [`CapabilityContext`]
//!   composition helper.
//! - [`hlist`] — heterogeneous-list type machinery used by the dependency
//!   typestate.
//! - [`error`] — typed error enums ([`LoaderError`], [`ConfigError`]) raised by
//!   the registry and builder.
//!
//! # Feature flags
//!
//! This crate has no Cargo features; everything compiles into every build.

/// Type-erased wrappers used by the typed registry.
pub mod any;
mod asset;
/// Fluent typed builder that enforces dependency ordering at compile time.
pub mod builder;
/// `Has*` capability traits and the [`CapabilityContext`] helper.
pub mod capabilities;
/// Runtime context handed to extensions on registration and router build.
pub mod context;
/// Typed error enums raised by the registry, validator, and builder.
pub mod error;
/// Heterogeneous-list type machinery used by the dependency typestate.
pub mod hlist;
/// Static metadata, schema-source, and role-definition value types.
pub mod metadata;
/// Schema migration value type.
pub mod migration;
/// Dynamic extension registry storing `Arc<dyn Extension>` values.
pub mod registry;
/// Router and site-auth configuration value types.
pub mod router;
/// Process-level extension injection (fallback for when `inventory` is
/// unavailable).
pub mod runtime_config;
mod traits;
/// Compile-time-checked typed sub-traits for extensions.
pub mod typed;
/// Typed registry, indexed by both concrete type and string ID.
pub mod typed_registry;
/// Type-level extension identifiers and dependency lists.
pub mod types;

/// Asset declaration value types — used by extensions that ship CSS, HTML
/// fragments, fonts, images, or JavaScript that the host must copy into
/// the web distribution.
pub use asset::{AssetDefinition, AssetDefinitionBuilder, AssetPaths, AssetType};
/// Runtime context handed to extensions during router resolution.
pub use context::{DynExtensionContext, ExtensionContext};
/// Typed error enums raised by extension loading and configuration.
pub use error::{ConfigError, LoaderError};
/// Static metadata block that every extension publishes.
pub use metadata::{ExtensionMetadata, ExtensionRole, SchemaDefinition, SchemaSource, SeedSource};
/// Schema migration descriptor.
pub use migration::Migration;
/// Dynamic registry types.
pub use registry::{ExtensionRegistration, ExtensionRegistry};
/// Router definition and authentication configuration value types.
pub use router::{ExtensionRouter, ExtensionRouterConfig, SiteAuthConfig};
/// The core `Extension` trait every extension must implement.
pub use traits::Extension;

/// Type-erased extension wrappers used to bridge typed and dyn-compatible
/// registries.
pub use any::{AnyExtension, ApiExtensionWrapper, ExtensionWrapper, SchemaExtensionWrapper};
/// Fluent builder for assembling a [`TypedExtensionRegistry`].
pub use builder::ExtensionBuilder;
/// Capability traits used to compose extension contexts.
pub use capabilities::{
    CapabilityContext, FullContext, HasAnalytics, HasConfig, HasDatabase, HasEventBus,
    HasExtension, HasFingerprint, HasHttpClient, HasRouteClassifier, HasUserService,
};
/// Heterogeneous-list type machinery (used by the typed builder's
/// dependency-list typestate).
pub use hlist::{Contains, NotSame, Subset, TypeList};
/// Typed sub-traits — `SchemaExtensionTyped`, `ApiExtensionTyped`,
/// `JobExtensionTyped`, `ConfigExtensionTyped`, `ProviderExtensionTyped`.
pub use typed::{
    ApiExtensionTyped, ApiExtensionTypedDyn, ConfigExtensionTyped, JobExtensionTyped,
    ProviderExtensionTyped, SchemaDefinitionTyped, SchemaExtensionTyped, SchemaSourceTyped,
};
/// Typed registry that tracks registrations by both ID and concrete type.
pub use typed_registry::{RESERVED_PATHS, TypedExtensionRegistry};
/// Static-type identifiers and dependency machinery used by typed
/// extensions.
pub use types::{
    Dependencies, DependencyList, ExtensionMeta, ExtensionType, MissingDependency, NoDependencies,
};

/// Curated re-exports of the most commonly used items, suitable for `use
/// systemprompt_extension::prelude::*;`.
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
