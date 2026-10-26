//! File and module discovery infrastructure for systemprompt.io.
//!
//! Reads the active services-config (with include resolution and
//! deduplication), loads profile YAML, writes agent files, and discovers
//! extension manifests under `extensions/`. Sits one level above
//! [`systemprompt_config`] in the dependency graph so that domain crates
//! never need to know how the on-disk layout is structured.
//!
//! # Modules
//!
//! - [`config_loader`] — loads and merges `services.yaml` and its includes.
//! - [`config_writer`] — creates, edits, and deletes agent files.
//! - [`extension_loader`] / [`extension_binary_index`] — discover extension
//!   manifests and resolve binary paths.
//! - [`module_loader`] — `inventory`-driven extension discovery for the
//!   compiled-in extension trait registry.
//! - [`profile_loader`] — reads, validates, and writes profile YAML.
//! - [`bundle`] — signed services bundles: fetch, verify, compose, and the boot
//!   path that installs the active services root.
//! - [`vertex_discovery`] — boot-time catalog discovery: each provider's
//!   credential decides which `CatalogSource` may list it, and every priced,
//!   serverless model it returns is folded into the provider registry.
//! - [`boot`] — [`BootContext`], the owned profile, secrets, resolved config
//!   and services root the entry crates build once at startup.
//! - [`services_state`] — [`ServicesState`], the owned services config, its
//!   resolved catalog and the discovery report, built once at boot and handed
//!   to every consumer.
//! - [`error`] — public error types ([`ConfigLoadError`], [`ConfigWriteError`],
//!   [`ExtensionLoadError`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod boot;
pub mod bundle;
pub mod config_loader;
pub mod config_writer;
pub mod error;
pub mod extension_binary_index;
pub mod extension_loader;
pub mod module_loader;
pub mod profile_loader;
pub mod services_state;
pub mod subprocess;
pub mod vertex_discovery;

pub use boot::BootContext;
pub use bundle::{BundleError, BundleResult, ServicesSourceBootstrap};
pub use config_loader::ConfigLoader;
pub use config_writer::ConfigWriter;
pub use error::{
    ConfigLoadError, ConfigLoadResult, ConfigWriteError, ConfigWriteResult, ExtensionLoadError,
    ExtensionLoadResult, ProfileLoadError, ProfileLoadResult,
};
pub use extension_binary_index::ExtensionBinaryIndex;
pub use extension_loader::{ExtensionLoader, ExtensionValidationResult};
pub use module_loader::ModuleLoader;
pub use profile_loader::ProfileLoader;
pub use services_state::{DiscoveryFuture, ServicesState, catalog_context};
