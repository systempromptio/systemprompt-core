//! The named registration list of instance-side extensions.
//!
//! The composition names its members: nothing is collected at link time.
//! [`REGISTERED`] lists the extensions that ship with the device, each under
//! its extension id, and
//! [`SpExtensions::registered`](crate::extension::SpExtensions::registered)
//! builds them from it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_identifiers::engine::ExtensionId;

use crate::extension::extension::SpExtension;
use crate::governance::GovernanceExtension;
use crate::members::brain::{BRAIN_EXTENSION, BrainExtension};
use crate::members::drive::DriveExtension;
use crate::members::notifications::NotificationsExtension;
use crate::members::slack::SlackExtension;

/// One registered extension type: its id and the factory that builds it with
/// `Default`.
#[derive(Clone)]
pub struct SpExtensionRegistration {
    pub id: ExtensionId,
    pub factory: fn() -> Arc<dyn SpExtension>,
}

impl SpExtensionRegistration {
    #[must_use]
    pub const fn of<E: SpExtension + Default>(id: ExtensionId) -> Self {
        Self {
            id,
            factory: build::<E>,
        }
    }
}

fn build<E: SpExtension + Default>() -> Arc<dyn SpExtension> {
    Arc::new(E::default())
}

impl fmt::Debug for SpExtensionRegistration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpExtensionRegistration")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

pub const REGISTERED: [SpExtensionRegistration; 5] = [
    SpExtensionRegistration::of::<GovernanceExtension>(GovernanceExtension::ID),
    SpExtensionRegistration::of::<DriveExtension>(DriveExtension::ID),
    SpExtensionRegistration::of::<BrainExtension>(BRAIN_EXTENSION),
    SpExtensionRegistration::of::<NotificationsExtension>(NotificationsExtension::ID),
    SpExtensionRegistration::of::<SlackExtension>(SlackExtension::ID),
];
