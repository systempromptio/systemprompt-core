//! The `notifications` extension: one handle shared with the shell, the
//! manifest applier that points it at the applied instance or withholds it,
//! and the feed service.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_device::client::HttpClient;
use crate::extension::{
    AppliedInstance, ApplyError, BackgroundService, ExtensionId, ExtensionMetadata,
    ManifestApplier, SpContext, SpExtension, extension_id, register_sp_extension,
};

use crate::members::notifications::client::NotificationsClient;
use crate::members::notifications::handle::NotificationsHandle;
use crate::members::notifications::service::{NotificationsService, PollCadence};

/// The reader's notifications as an instance-side extension.
///
/// `provide` inserts its [`NotificationsHandle`] into `ctx.handles`; its
/// applier follows the applied instance only while that instance enables
/// `notifications` and withholds the handle otherwise; its service polls the
/// feed with the default [`PollCadence`].
#[derive(Debug, Default)]
pub struct NotificationsExtension {
    handle: NotificationsHandle,
}

impl NotificationsExtension {
    pub const ID: ExtensionId = extension_id!("notifications");

    pub const fn handle(&self) -> &NotificationsHandle {
        &self.handle
    }
}

impl SpExtension for NotificationsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata::new(Self::ID, "Notifications")
    }

    fn provide(&self, ctx: &SpContext) {
        ctx.handles.insert(Arc::new(self.handle.clone()));
    }

    fn appliers(&self, ctx: &SpContext) -> Vec<Arc<dyn ManifestApplier>> {
        vec![Arc::new(FollowInstance {
            handle: self.handle.clone(),
            http: ctx.http.clone(),
        })]
    }

    fn services(&self, _ctx: &SpContext) -> Vec<Arc<dyn BackgroundService>> {
        vec![Arc::new(NotificationsService::new(
            self.handle.clone(),
            PollCadence::default(),
        ))]
    }
}

register_sp_extension!(NotificationsExtension);

struct FollowInstance {
    handle: NotificationsHandle,
    http: HttpClient,
}

impl ManifestApplier for FollowInstance {
    fn apply(&self, instance: &AppliedInstance) -> Result<(), ApplyError> {
        let client = NotificationsClient::new(
            &instance.client,
            self.http.clone(),
            Arc::clone(&instance.credentials),
        );
        self.handle.follow(instance.account.clone(), client);
        Ok(())
    }

    fn clear(&self) {
        self.handle.withhold();
    }
}
