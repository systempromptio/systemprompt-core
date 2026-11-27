//! The instance-side extension contract.
//!
//! A feature that talks to an instance is an [`SpExtension`]: a
//! `Default`-constructed type named in the composition's registration list and
//! built at startup by [`SpExtensions::from_registrations`]. Each contribution
//! method receives the [`SpContext`] the composition root built: the accounts,
//! the instance HTTP client, the paths, the extension's own settings, the
//! shared handles and the instance's enablement. An extension contributes its
//! engine half, [`ManifestApplier`]s that follow the applied instance, and
//! [`BackgroundService`]s the composition root runs as owned tasks;
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
mod contract;
mod discovery;
mod enablement;
mod error;
mod handles;
mod policy;
mod register;
mod service;
mod settings;

pub use applier::{AppliedInstance, ApplyError, ManifestApplier, ManifestAppliers};
pub use context::{HostInfo, SpContext, SpPaths};
pub use contract::SpExtension;
pub use discovery::SpExtensions;
pub use enablement::Enablement;
pub use error::ExtensionError;
pub use handles::Handles;
pub use policy::ExtensionPolicy;
pub use register::SpExtensionRegistration;
pub use service::BackgroundService;
pub use settings::{ExtensionSettings, SettingsAccess, SettingsError};
pub use systemprompt_engine::run::extension::{
    Contributed, EnabledExtensions, EngineExtension, ExtensionMetadata, ResolveError, Resolved,
};
pub use systemprompt_identifiers::engine::ExtensionId;
pub use systemprompt_identifiers::extension_id;
pub use systemprompt_traits::CancellationToken;
