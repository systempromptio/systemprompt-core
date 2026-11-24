//! The admin-bootstrap view the CLI host builds before any kernel state.
//!
//! The users member contributes the one constructor
//! (`Contributions::admin_bootstrap`) that turns an open database handle into
//! a [`DynAdminBootstrap`]; the CLI host reads it from the admitted set, from
//! a core-standing `users` member only, for `admin bootstrap`, `admin session
//! login`, CLI session creation and `infra db assign-admin`. The view finds a
//! user by name or email, creates a user, grants the admin role and confirms a
//! session row exists. It cannot demote, delete, ban, merge, list or update a
//! user, and it reads no credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{SessionId, UserId};

/// A user as the admin-bootstrap flows read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminAccount {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub status: String,
    pub roles: Vec<String>,
    pub active: bool,
    pub service_account: bool,
}

impl AdminAccount {
    pub const ADMIN_ROLE: &'static str = "admin";

    #[must_use]
    pub fn is_admin(&self) -> bool {
        self.roles.iter().any(|role| role == Self::ADMIN_ROLE)
    }
}

/// How an admin grant treats the roles the user already holds: `Added` keeps
/// them and adds `admin`; `Only` leaves `admin` as the one role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminGrant {
    Added,
    Only,
}

/// The outcome of promoting a user named by id, name or email.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminPromotion {
    Promoted(AdminAccount),
    AlreadyAdmin(AdminAccount),
    NotFound,
}

/// The user store refused an admin-bootstrap step; the store's error is the
/// source.
#[derive(Debug, thiserror::Error)]
#[error("the user store refused the admin bootstrap step")]
pub struct AdminBootstrapError(#[source] pub Box<dyn std::error::Error + Send + Sync>);

/// Finds, creates and promotes the platform admin. `#[async_trait]` keeps
/// the trait `dyn`-compatible for [`DynAdminBootstrap`].
#[async_trait]
pub trait AdminBootstrap: Send + Sync + Debug {
    async fn find_by_name(&self, name: &str) -> Result<Option<AdminAccount>, AdminBootstrapError>;

    async fn find_by_email(&self, email: &str)
    -> Result<Option<AdminAccount>, AdminBootstrapError>;

    async fn create(
        &self,
        name: &str,
        email: &str,
        full_name: Option<&str>,
    ) -> Result<AdminAccount, AdminBootstrapError>;

    async fn create_if_absent(
        &self,
        name: &str,
        email: &str,
    ) -> Result<Option<AdminAccount>, AdminBootstrapError>;

    async fn grant_admin(
        &self,
        user: &UserId,
        grant: AdminGrant,
    ) -> Result<AdminAccount, AdminBootstrapError>;

    async fn promote(&self, reference: &str) -> Result<AdminPromotion, AdminBootstrapError>;

    async fn session_exists(&self, session: &SessionId) -> Result<bool, AdminBootstrapError>;
}

pub type DynAdminBootstrap = Arc<dyn AdminBootstrap>;
