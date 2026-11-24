//! The service-account directory an in-process core service runs its model
//! calls as.
//!
//! [`ServiceAccounts::ensure`] creates the non-human account a core service
//! names (active, the plain `user` role, `account_kind = service`) or
//! returns the one already holding its email. It reads nothing else and can
//! grant no role. The users member provides it sealed
//! ([`DynServiceAccounts`]); the API host reads it to resolve the brain's
//! gateway principal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;

/// The account a core service asks to run as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceAccountRequest {
    pub name: String,
    pub email: String,
    pub created_by: UserId,
}

/// The account the directory holds under the requested email.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceAccount {
    pub id: UserId,
    pub roles: Vec<String>,
    pub service: bool,
}

/// The user store refused to ensure the account; its error is the source.
#[derive(Debug, thiserror::Error)]
#[error("the user store refused to ensure the service account")]
pub struct ServiceAccountError(#[source] pub Box<dyn std::error::Error + Send + Sync>);

/// Ensures a core service's account. `#[async_trait]` keeps the trait
/// `dyn`-compatible for [`DynServiceAccounts`].
#[async_trait]
pub trait ServiceAccounts: Send + Sync + Debug {
    async fn ensure(
        &self,
        request: ServiceAccountRequest,
    ) -> Result<ServiceAccount, ServiceAccountError>;
}

pub type DynServiceAccounts = Arc<dyn ServiceAccounts>;
