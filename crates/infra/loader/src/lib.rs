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
//! - [`extension_loader`] / [`extension_registry`] — discover extension
//!   manifests and resolve binary paths.
//! - [`module_loader`] — `inventory`-driven extension discovery for the
//!   compiled-in extension trait registry.
//! - [`profile_loader`] — reads, validates, and writes profile YAML.
//! - [`error`] — public error types ([`ConfigLoadError`], [`ConfigWriteError`],
//!   [`ExtensionLoadError`]).
//!
//! # Feature flags
//!
//! - `expose-internals` — exposes test-only entry points (notably
//!   `ConfigLoader::load_from_content`) to dependent crates that exercise the
//!   loader from outside `cfg(test)`. Off by default.

pub mod config_loader;
pub mod config_writer;
pub mod error;
pub mod extension_loader;
pub mod extension_registry;
pub mod module_loader;
mod modules;
pub mod profile_loader;

pub use config_loader::ConfigLoader;
pub use config_writer::ConfigWriter;
pub use error::{
    ConfigLoadError, ConfigLoadResult, ConfigWriteError, ConfigWriteResult, ExtensionLoadError,
    ExtensionLoadResult, ProfileLoadError, ProfileLoadResult,
};
pub use extension_loader::{ExtensionLoader, ExtensionValidationResult};
pub use extension_registry::ExtensionRegistry;
pub use module_loader::ModuleLoader;
pub use profile_loader::ProfileLoader;
