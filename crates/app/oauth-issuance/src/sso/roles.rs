//! Role resolution between an admitted identity and the session it gets.
//!
//! The policy maps directory groups to roles, but `users.roles` is also
//! edited out-of-band by operators; overwriting it with the directory set on
//! every sign-in would drop those grants. A member contributes a
//! [`RoleResolver`] to [`SsoSlot`](crate::contract::SsoSlot) to merge the two;
//! without one the directory set wins.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt::Debug;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_traits::sso::SsoError;

/// Decides the roles a resolved user signs in with, given the roles the
/// identity policy derived from the directory. Held as
/// `Arc<dyn RoleResolver>` by the driver.
#[async_trait]
pub trait RoleResolver: Send + Sync + Debug {
    async fn resolve(
        &self,
        user_id: &UserId,
        directory_roles: &[String],
    ) -> Result<Vec<String>, SsoError>;
}

/// The default resolver: the directory's roles, unchanged.
#[derive(Debug, Clone, Copy, Default)]
pub struct DirectoryOnlyRoleResolver;

#[async_trait]
impl RoleResolver for DirectoryOnlyRoleResolver {
    async fn resolve(
        &self,
        _user_id: &UserId,
        directory_roles: &[String],
    ) -> Result<Vec<String>, SsoError> {
        Ok(directory_roles.to_vec())
    }
}
