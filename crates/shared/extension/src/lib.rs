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
//! - [`build`] — build-script helper that generates `Extension::migrations()`
//!   from `schema/migrations/*.sql` files, paired with the
//!   [`extension_migrations!`] macro.
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
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod any;
mod asset;
#[expect(
    clippy::panic,
    clippy::manual_assert,
    clippy::print_stdout,
    reason = "build-script support code: a panic is the correct way to abort a build, and \
              println! is how a build script emits cargo directives"
)]
pub mod build;
pub mod builder;
pub mod capabilities;
pub mod context;
pub mod error;
pub mod frame_options;
pub mod hlist;
pub mod metadata;
pub mod migration;
pub mod registry;
pub mod router;
pub mod runtime_config;
pub mod seed;
mod traits;
pub mod typed;
pub mod typed_registry;
pub mod types;

pub use asset::{AssetDefinition, AssetDefinitionBuilder, AssetPaths, AssetType};
pub use context::{DynExtensionContext, ExtensionContext};
pub use error::{ConfigError, LoaderError};
pub use frame_options::{FrameOptions, FrameOptionsOverride, stamp_frame_options};
pub use metadata::{ExtensionMetadata, ExtensionRole, SchemaDefinition};
pub use migration::Migration;
pub use registry::{ExtensionRegistration, ExtensionRegistry};
pub use router::{ExtensionRouter, ExtensionRouterConfig, SiteAuthConfig};
pub use seed::Seed;
pub use traits::Extension;

pub use any::{AnyExtension, ApiExtensionWrapper, ExtensionWrapper, SchemaExtensionWrapper};
pub use builder::ExtensionBuilder;
pub use capabilities::{
    CapabilityContext, FullContext, HasAnalytics, HasConfig, HasDatabase, HasEventBus,
    HasExtension, HasFingerprint, HasHttpClient, HasRouteClassifier, HasUserService,
};
pub use hlist::{Subset, TypeList};
pub use typed::{
    ApiExtensionTyped, ApiExtensionTypedDyn, ConfigExtensionTyped, JobExtensionTyped,
    ProviderExtensionTyped, SchemaDefinitionTyped, SchemaExtensionTyped,
};
pub use typed_registry::{RESERVED_PATHS, TypedExtensionRegistry};
pub use types::{Dependencies, DependencyList, ExtensionMeta, ExtensionType, NoDependencies};

pub mod prelude {
    pub use crate::asset::{AssetDefinition, AssetDefinitionBuilder, AssetPaths, AssetType};
    pub use crate::context::{DynExtensionContext, ExtensionContext};
    pub use crate::error::{ConfigError, LoaderError};
    pub use crate::registry::ExtensionRegistry;
    pub use crate::seed::Seed;
    pub use crate::{
        Extension, ExtensionMetadata, ExtensionRole, ExtensionRouter, FrameOptions, Migration,
        SchemaDefinition, SiteAuthConfig, extension_migrations, register_extension,
        stamp_frame_options,
    };

    pub use crate::any::AnyExtension;
    pub use crate::builder::ExtensionBuilder;
    pub use crate::capabilities::{
        CapabilityContext, FullContext, HasConfig, HasDatabase, HasEventBus, HasExtension,
        HasHttpClient,
    };

    pub use crate::hlist::{Subset, TypeList};
    pub use crate::typed::{
        ApiExtensionTyped, ApiExtensionTypedDyn, ConfigExtensionTyped, JobExtensionTyped,
        ProviderExtensionTyped, SchemaDefinitionTyped, SchemaExtensionTyped,
    };
    pub use crate::typed_registry::{RESERVED_PATHS, TypedExtensionRegistry};
    pub use crate::types::{
        Dependencies, DependencyList, ExtensionMeta, ExtensionType, NoDependencies,
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
