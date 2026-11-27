//! The `drive` extension.
//!
//! The handle shared with the shell, the manifest applier that points it at
//! the applied instance or withholds it, the agent tools of its engine half
//! and the service that keeps the local folder sync on the applied account.
//!
//! An instance that does not enable `drive` for this host, or no applied
//! instance at all, leaves the handle withheld: no client, no tools offered,
//! the sync stopped.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod engine;
mod follow;
mod handle;
mod kind;
mod settings;
mod tools;

use std::path::PathBuf;
use std::sync::Arc;

use sp_client::HttpClient;
use sp_extension::{
    AppliedInstance, ApplyError, BackgroundService, EngineExtension, ExtensionId,
    ExtensionMetadata, ManifestApplier, SpContext, SpExtension, extension_id,
    register_sp_extension,
};

pub use engine::DriveEngine;
pub use follow::DriveFollower;
pub use handle::{DriveHandle, DriveWithheld};
pub use kind::DocumentKind;
pub use settings::DriveSettings;
pub use tools::{PERMISSION, TOOL_NAMES};

use crate::client::DriveClient;
use crate::sync::default_drive_root;

/// The Drive as an instance-side extension.
///
/// `provide` inserts its [`DriveHandle`] into `ctx.handles`; its applier
/// follows the applied instance while that instance enables `drive` and
/// withholds the handle otherwise; its engine half offers the Drive tools; its
/// [`DriveFollower`] service keeps the local folder sync on the applied
/// account, in the folder named by [`DriveSettings`].
#[derive(Debug, Default)]
pub struct DriveExtension {
    handle: DriveHandle,
}

impl DriveExtension {
    pub const ID: ExtensionId = extension_id!("drive");

    pub const fn handle(&self) -> &DriveHandle {
        &self.handle
    }

    pub fn default_folder(ctx: &SpContext) -> Option<PathBuf> {
        ctx.paths
            .documents
            .as_deref()
            .map(|documents| default_drive_root(documents, ctx.host.product))
    }

    pub(crate) const fn describe() -> ExtensionMetadata {
        ExtensionMetadata::new(Self::ID, "Drive")
    }
}

impl SpExtension for DriveExtension {
    fn metadata(&self) -> ExtensionMetadata {
        Self::describe()
    }

    fn provide(&self, ctx: &SpContext) {
        ctx.handles.insert(Arc::new(self.handle.clone()));
    }

    fn engine(&self, _ctx: &SpContext) -> Option<Arc<dyn EngineExtension>> {
        Some(Arc::new(DriveEngine::new(self.handle.clone())))
    }

    fn appliers(&self, ctx: &SpContext) -> Vec<Arc<dyn ManifestApplier>> {
        vec![Arc::new(FollowInstance {
            handle: self.handle.clone(),
            http: ctx.http.clone(),
        })]
    }

    fn services(&self, ctx: &SpContext) -> Vec<Arc<dyn BackgroundService>> {
        vec![Arc::new(DriveFollower::new(
            self.handle.clone(),
            ctx.accounts.clone(),
            ctx.http.clone(),
            ctx.settings_for(&Self::ID),
            Self::default_folder(ctx),
        ))]
    }
}

register_sp_extension!(DriveExtension);

struct FollowInstance {
    handle: DriveHandle,
    http: HttpClient,
}

impl ManifestApplier for FollowInstance {
    fn apply(&self, instance: &AppliedInstance) -> Result<(), ApplyError> {
        let client = DriveClient::new(
            &instance.client,
            self.http.clone(),
            Arc::clone(&instance.credentials),
        );
        self.handle
            .follow(instance.account.clone(), instance.client.clone(), client);
        Ok(())
    }

    fn clear(&self) {
        self.handle.withhold();
    }
}
