//! Built-in role name constants.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::permission::Permission;

#[derive(Debug, Copy, Clone)]
pub struct BaseRoles;

impl BaseRoles {
    pub const ANONYMOUS: &'static str = "anonymous";
    pub const USER: &'static str = "user";
    pub const ADMIN: &'static str = "admin";

    #[must_use]
    pub fn permission_for(roles: &[String]) -> Permission {
        if roles.iter().any(|role| role == Self::ADMIN) {
            Permission::Admin
        } else {
            Permission::User
        }
    }
}
