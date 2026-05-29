//! Background jobs for the users domain.
//!
//! Currently exposes [`CleanupAnonymousUsersJob`], which prunes stale
//! anonymous accounts on a schedule.

pub mod cleanup_anonymous_users;

pub use cleanup_anonymous_users::CleanupAnonymousUsersJob;
