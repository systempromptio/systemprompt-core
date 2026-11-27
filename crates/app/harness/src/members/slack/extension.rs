//! The `slack` extension: a handle over the reader's notifications and the
//! manifest applier that admits or withholds it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use sp_extension::{
    AppliedInstance, ApplyError, ExtensionId, ExtensionMetadata, ManifestApplier, SpContext,
    SpExtension, extension_id, register_sp_extension,
};
use sp_notifications::{NotificationsExtension, NotificationsHandle};

use crate::handle::{Shared, SlackHandle};

/// Slack as an instance-side extension, depending on `notifications`.
///
/// `provide` inserts its [`SlackHandle`] into `ctx.handles`; the handle
/// reads the notifications handle found there (looked up again when the
/// appliers are built, after every extension provided). Its applier admits
/// the handle while the applied instance enables `slack` and withholds it
/// otherwise. It contributes no service: it never calls Slack or the instance.
pub struct SlackExtension {
    shared: Arc<Shared>,
}

impl SlackExtension {
    pub const ID: ExtensionId = extension_id!("slack");

    pub fn handle(&self) -> SlackHandle {
        SlackHandle::new(Arc::clone(&self.shared))
    }

    fn attach(&self, ctx: &SpContext) {
        if self.shared.is_attached() {
            return;
        }
        if let Some(notifications) = ctx.handles.get::<NotificationsHandle>() {
            self.shared
                .attach(NotificationsHandle::clone(&notifications));
        }
    }
}

impl Default for SlackExtension {
    fn default() -> Self {
        Self {
            shared: Arc::new(Shared::new()),
        }
    }
}

impl fmt::Debug for SlackExtension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SlackExtension")
            .field("handle", &self.handle())
            .finish()
    }
}

impl SpExtension for SlackExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata::new(Self::ID, "Slack").depends_on(NotificationsExtension::ID)
    }

    fn provide(&self, ctx: &SpContext) {
        self.attach(ctx);
        ctx.handles.insert(Arc::new(self.handle()));
    }

    fn appliers(&self, ctx: &SpContext) -> Vec<Arc<dyn ManifestApplier>> {
        self.attach(ctx);
        if !self.shared.is_attached() {
            tracing::warn!(
                extension = %Self::ID,
                "no notifications handle was provided; slack stays withheld"
            );
        }
        vec![Arc::new(Admission {
            shared: Arc::clone(&self.shared),
        })]
    }
}

register_sp_extension!(SlackExtension);

struct Admission {
    shared: Arc<Shared>,
}

impl ManifestApplier for Admission {
    fn apply(&self, _instance: &AppliedInstance) -> Result<(), ApplyError> {
        self.shared.admit(true);
        Ok(())
    }

    fn clear(&self) {
        self.shared.admit(false);
    }
}
