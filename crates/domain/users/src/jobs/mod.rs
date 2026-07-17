//! Background jobs for the users domain.
//!
//! Currently exposes [`CleanupAnonymousUsersJob`], which prunes stale
//! anonymous accounts on a schedule.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod cleanup_anonymous_users;

pub use cleanup_anonymous_users::CleanupAnonymousUsersJob;
