//! Background jobs for the users domain.
//!
//! [`CleanupAnonymousUsersJob`] prunes stale anonymous accounts and
//! [`UserRateLimitPruneJob`] drops elapsed rate-limit windows, both hourly;
//! [`CleanupInactiveSessionsJob`] ends sessions idle past `inactive_hours`
//! every ten minutes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod cleanup_anonymous_users;
pub mod cleanup_inactive_sessions;
pub mod user_rate_limit_prune;

pub use cleanup_anonymous_users::CleanupAnonymousUsersJob;
pub use cleanup_inactive_sessions::CleanupInactiveSessionsJob;
pub use user_rate_limit_prune::UserRateLimitPruneJob;
