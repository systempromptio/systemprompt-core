//! Authentication and role-management provider traits.
//!
//! `UserProvider` is dispatched as a trait object (`dyn UserProvider`), so it
//! uses `#[async_trait]`; native `async fn` in traits is not yet
//! `dyn`-compatible. `RoleProvider` is only used through concrete types and
//! declares native `async` methods.
//!
//! `is_service_account` marks a non-human service account: it never receives an
//! interactive credential (session login, passkey, bridge code, authorization
//! code), and `ensure_interactive` refuses it on every interactive sign-in
//! path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::future::Future;
use systemprompt_identifiers::UserId;

use crate::BoxedSource;

pub type AuthResult<T> = Result<T, AuthProviderError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AuthProviderError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("User not found")]
    UserNotFound,

    #[error("Invalid token")]
    InvalidToken,

    #[error("Token expired")]
    TokenExpired,

    #[error("Insufficient permissions")]
    InsufficientPermissions,

    #[error("Service accounts cannot sign in interactively")]
    ServiceAccountInteractiveLogin,

    #[error("Federated sign-in refused: {}", FederatedRefusal::EmailInUse)]
    FederatedEmailInUse,

    #[error("Federated sign-in refused: {}", FederatedRefusal::TargetIneligible)]
    FederatedTargetIneligible,

    #[error("Federated sign-in refused: {}", FederatedRefusal::NotProvisioned)]
    FederatedNotProvisioned,

    #[error("Internal error: {0}")]
    Internal(#[source] BoxedSource),
}

impl AuthProviderError {
    #[must_use]
    pub const fn federated_refusal(&self) -> Option<FederatedRefusal> {
        match self {
            Self::FederatedEmailInUse => Some(FederatedRefusal::EmailInUse),
            Self::FederatedTargetIneligible => Some(FederatedRefusal::TargetIneligible),
            Self::FederatedNotProvisioned => Some(FederatedRefusal::NotProvisioned),
            _ => None,
        }
    }
}

impl From<FederatedRefusal> for AuthProviderError {
    fn from(refusal: FederatedRefusal) -> Self {
        match refusal {
            FederatedRefusal::EmailInUse => Self::FederatedEmailInUse,
            FederatedRefusal::TargetIneligible => Self::FederatedTargetIneligible,
            FederatedRefusal::NotProvisioned => Self::FederatedNotProvisioned,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub roles: Vec<String>,
    pub is_active: bool,
    pub is_service_account: bool,
}

impl AuthUser {
    pub const fn ensure_interactive(&self) -> AuthResult<()> {
        if self.is_service_account {
            return Err(AuthProviderError::ServiceAccountInteractiveLogin);
        }
        Ok(())
    }
}

/// Federated-identity claim payload passed to
/// [`UserProvider::resolve_federated`].
///
/// Carries only the OIDC fields needed to seed a freshly federated user — the
/// trait stays free of any concrete JWT type so it can live in
/// `systemprompt-traits` without taking a dependency on `systemprompt-models`.
#[derive(Debug, Clone, Default)]
pub struct FederatedIdentityClaims {
    pub email: Option<String>,
    pub email_verified: bool,
    pub name: Option<String>,
    pub preferred_username: Option<String>,
    pub roles: Vec<String>,
}

/// Whether an inbound chat-platform sender resolves to a linkable identity.
///
/// This is the identity-linking rule, stated once: a sender is `Linked` only
/// when the platform verified the claims (for Slack, a workspace profile with
/// a confirmed email — an unconfirmed address would let anyone who can set it
/// claim the account that owns it). Anything less is `Unlinked`, whose empty
/// claims land the sender on a fresh, role-less first-touch user that no rule
/// grants anything to — never on an existing account.
#[derive(Debug, Clone, Default)]
pub enum SenderIdentity {
    Linked(FederatedIdentityClaims),
    #[default]
    Unlinked,
}

impl SenderIdentity {
    #[must_use]
    pub fn claims(&self) -> FederatedIdentityClaims {
        match self {
            Self::Linked(claims) => claims.clone(),
            Self::Unlinked => FederatedIdentityClaims::default(),
        }
    }
}

/// How an issuer's external identities may reach local accounts.
///
/// `link_by_email` (default `false`) lets an identity with no federated link
/// claim the ordinary account that already owns its verified email. With it
/// off, a verified email that an existing account owns refuses the sign-in.
/// `auto_provision` (default `true`) mints a fresh account for an identity
/// that matches nothing; with it off such an identity is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinkPolicy {
    #[serde(default)]
    pub link_by_email: bool,
    #[serde(default = "default_auto_provision")]
    pub auto_provision: bool,
}

const fn default_auto_provision() -> bool {
    true
}

impl Default for LinkPolicy {
    fn default() -> Self {
        Self {
            link_by_email: false,
            auto_provision: true,
        }
    }
}

impl LinkPolicy {
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Why [`UserProvider::resolve_federated`] refused an identity.
///
/// A link target is ineligible when it is not active, is a service account,
/// or holds any role other than `user` — those accounts are never claimed by
/// an external identity, whatever the policy says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FederatedRefusal {
    EmailInUse,
    TargetIneligible,
    NotProvisioned,
}

impl std::fmt::Display for FederatedRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::EmailInUse => "an existing account owns this email and linking is disabled",
            Self::TargetIneligible => "the account owning this email cannot be linked",
            Self::NotProvisioned => "no account exists and provisioning is disabled",
        })
    }
}

/// How a federated identity reached its local account.
///
/// Only a `Provisioned` account, or an `Existing` link the issuer itself
/// provisioned, may have the issuer's roles written onto it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FederatedLink {
    Existing { provisioned: bool },
    Linked,
    Provisioned,
}

impl FederatedLink {
    #[must_use]
    pub const fn issuer_owns_roles(self) -> bool {
        matches!(
            self,
            Self::Provisioned | Self::Existing { provisioned: true }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FederatedResolution {
    pub user_id: UserId,
    pub link: FederatedLink,
}

#[async_trait]
pub trait UserProvider: Send + Sync {
    async fn find_by_id(&self, id: &UserId) -> AuthResult<Option<AuthUser>>;
    async fn find_by_email(&self, email: &str) -> AuthResult<Option<AuthUser>>;
    async fn find_by_name(&self, name: &str) -> AuthResult<Option<AuthUser>>;
    async fn create_user(
        &self,
        name: &str,
        email: &str,
        full_name: Option<&str>,
    ) -> AuthResult<AuthUser>;
    async fn create_anonymous(&self, fingerprint: &str) -> AuthResult<AuthUser>;
    async fn assign_roles(&self, user_id: &UserId, roles: &[String]) -> AuthResult<()>;

    async fn resolve_federated(
        &self,
        issuer: &str,
        external_sub: &str,
        claims: &FederatedIdentityClaims,
        policy: &LinkPolicy,
    ) -> AuthResult<FederatedResolution>;

    async fn promote_anonymous(&self, source: &UserId, target: &UserId) -> AuthResult<u64>;
}

pub trait RoleProvider: Send + Sync {
    fn get_roles(&self, user_id: &UserId) -> impl Future<Output = AuthResult<Vec<String>>> + Send;
    fn assign_role(
        &self,
        user_id: &UserId,
        role: &str,
    ) -> impl Future<Output = AuthResult<()>> + Send;
    fn revoke_role(
        &self,
        user_id: &UserId,
        role: &str,
    ) -> impl Future<Output = AuthResult<()>> + Send;
    fn list_users_by_role(
        &self,
        role: &str,
    ) -> impl Future<Output = AuthResult<Vec<AuthUser>>> + Send;
}
