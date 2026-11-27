//! The members of the device composition that ship with it: Drive, Brain,
//! notifications and Slack. Each is the instance-side half of one extension
//! and is named in the registration list of [`crate::extension`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod brain;
pub mod drive;
pub mod notifications;
pub mod slack;
