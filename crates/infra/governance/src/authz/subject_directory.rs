//! Existence oracle for the `role` subject dimension.
//!
//! Roles live in the users domain's `users.roles` array, which this crate
//! does not query. The users domain registers a [`RoleDirectory`] through
//! [`register_role_directory!`][crate::register_role_directory]; the ingestion
//! service asks it which of the roles a rule names are held by nobody, so an
//! inert rule can be reported. Without a registered directory every role
//! mention is unverifiable and none is reported — the same posture group and
//! project dimensions have.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;

use super::error::AuthzError;

/// Answers which candidate roles no user holds.
///
/// `#[async_trait]` because the directory is held as an `Arc<dyn …>` on the
/// ingestion service, so the trait must stay `dyn`-compatible.
#[async_trait]
pub trait RoleDirectory: Send + Sync + Debug {
    async fn unknown_roles(&self, candidates: &[String]) -> Result<Vec<String>, AuthzError>;
}

pub type SharedRoleDirectory = Arc<dyn RoleDirectory>;

/// One inventory submission per
/// [`register_role_directory!`][crate::register_role_directory] call. The
/// factory receives the write pool the ingestion service runs on.
#[derive(Debug, Clone, Copy)]
pub struct RoleDirectoryRegistration {
    pub factory: fn(Arc<PgPool>) -> SharedRoleDirectory,
}

inventory::collect!(RoleDirectoryRegistration);

#[must_use]
pub fn discover_role_directory(pool: &Arc<PgPool>) -> Option<SharedRoleDirectory> {
    inventory::iter::<RoleDirectoryRegistration>()
        .next()
        .map(|reg| (reg.factory)(Arc::clone(pool)))
}

#[macro_export]
macro_rules! register_role_directory {
    ($factory:expr) => {
        ::inventory::submit! {
            $crate::authz::RoleDirectoryRegistration {
                factory: $factory,
            }
        }
    };
}
