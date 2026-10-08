//! Built-in role name constants.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Copy, Clone)]
pub struct BaseRoles;

impl BaseRoles {
    pub const ANONYMOUS: &'static str = "anonymous";
    pub const USER: &'static str = "user";
    pub const ADMIN: &'static str = "admin";
}
