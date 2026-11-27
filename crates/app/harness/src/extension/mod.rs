//! The instance-side extension contract.
//!
//! A feature that talks to an instance is an [`SpExtension`]: a
//! `Default`-constructed type registered with [`register_sp_extension!`] and
//! found at startup by [`SpExtensions::discover`]. Each contribution method
//! receives the [`SpContext`] the composition root built: the accounts, the
//! instance HTTP client, the paths, the extension's own settings, the shared
//! handles and the instance's enablement. An extension contributes its engine
//! half, [`ManifestApplier`]s that follow the applied instance, and
//! [`BackgroundService`]s the composition root runs through `harness-tasks`;
//! it never spawns work itself.
//!
//! Enablement fails closed: [`ExtensionPolicy`] reads the manifest's
//! `host_extensions` entry for this host, and an extension the instance did
//! not list is withheld unless it is required.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod applier;
mod context;
mod discovery;
mod enablement;
mod error;
mod extension;
mod handles;
mod policy;
mod register;
mod service;
mod settings;

#[doc(hidden)]
pub use inventory as __inventory;

pub use applier::{AppliedInstance, ApplyError, ManifestApplier, ManifestAppliers};
pub use context::{HostInfo, SpContext, SpPaths};
pub use discovery::SpExtensions;
pub use enablement::Enablement;
pub use error::ExtensionError;
pub use extension::SpExtension;
pub use handles::Handles;
pub use harness_engine::extension::{
    Contributed, EnabledExtensions, EngineExtension, ExtensionMetadata, ResolveError, Resolved,
};
pub use harness_ids::{ExtensionId, extension_id};
pub use harness_tasks::CancellationToken;
pub use policy::ExtensionPolicy;
pub use register::SpExtensionRegistration;
pub use service::BackgroundService;
pub use settings::{ExtensionSettings, SettingsAccess, SettingsError};
