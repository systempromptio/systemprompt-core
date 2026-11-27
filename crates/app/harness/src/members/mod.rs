//! The members of the device composition that ship with it.
//!
//! Drive, Brain, notifications and Slack are each the instance-side half of
//! one extension. [`REGISTERED`] names them and the governance extension, and
//! [`SpExtensions::registered`] builds the composition from that list.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod brain;
pub mod drive;
pub mod notifications;
pub mod slack;

use crate::extension::{ExtensionError, SpExtensionRegistration, SpExtensions};
use crate::governance::GovernanceExtension;
use crate::members::brain::{BRAIN_EXTENSION, BrainExtension};
use crate::members::drive::DriveExtension;
use crate::members::notifications::NotificationsExtension;
use crate::members::slack::SlackExtension;

pub const REGISTERED: [SpExtensionRegistration; 5] = [
    SpExtensionRegistration::of::<GovernanceExtension>(GovernanceExtension::ID),
    SpExtensionRegistration::of::<DriveExtension>(DriveExtension::ID),
    SpExtensionRegistration::of::<BrainExtension>(BRAIN_EXTENSION),
    SpExtensionRegistration::of::<NotificationsExtension>(NotificationsExtension::ID),
    SpExtensionRegistration::of::<SlackExtension>(SlackExtension::ID),
];

impl SpExtensions {
    pub fn registered() -> Result<Self, ExtensionError> {
        Self::from_registrations(&REGISTERED)
    }
}
