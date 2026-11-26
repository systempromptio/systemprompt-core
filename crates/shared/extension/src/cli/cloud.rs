//! The cloud capability the kernel CLI reaches through the cloud member.
//!
//! The kernel CLI never links the cloud client. Signing in, the cloud
//! account's email, the local tenant store and the operator token exchange
//! are behind [`CloudAccess`], which the cloud member implements and the
//! composition hands the CLI when it is built with the facade feature
//! `cloud`. Without it every cloud path is a [`CloudAccessError::Absent`]
//! refusal naming that feature; none falls back to local execution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use systemprompt_identifiers::{Email, SessionToken, TenantId};
use systemprompt_traits::BoxedSource;

use super::command::BoxFuture;

/// The cloud account and tenant routing a cloud member provides to the
/// kernel CLI. Object-safe: the composition hands it over as
/// [`DynCloudAccess`].
pub trait CloudAccess: Send + Sync {
    fn account(&self) -> BoxFuture<'_, Result<CloudAccount, CloudAccessError>>;

    fn credentials_present(&self) -> bool;

    fn tenant_route(
        &self,
        tenants_path: &Path,
        tenant: &TenantId,
    ) -> Result<TenantRoute, CloudAccessError>;

    fn operator_tenant_token<'a>(
        &'a self,
        hostname: &'a str,
    ) -> BoxFuture<'a, Result<SessionToken, CloudAccessError>>;

    fn tenant_base_url(&self, hostname: &str) -> String;
}

pub type DynCloudAccess = Arc<dyn CloudAccess>;

/// What loading the cloud credentials found.
#[derive(Debug)]
pub enum CloudAccount {
    SignedIn {
        user_email: Email,
        token_expired: bool,
    },
    LocalOnly {
        reason: BoxedSource,
    },
}

impl CloudAccount {
    #[must_use]
    pub const fn user_email(&self) -> Option<&Email> {
        match self {
            Self::SignedIn { user_email, .. } => Some(user_email),
            Self::LocalOnly { .. } => None,
        }
    }
}

/// A synced tenant's routing facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantRoute {
    pub hostname: Option<String>,
    pub self_managed: bool,
    pub provider: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CloudAccessError {
    #[error(
        "{action} needs the cloud member, which this build does not include; rebuild with the \
         facade feature `cloud`"
    )]
    Absent { action: &'static str },

    #[error("{action} needs the cloud member, which profile '{profile}' disables")]
    Disabled {
        action: &'static str,
        profile: String,
    },

    #[error("Failed to load tenants from {path}. Run 'systemprompt cloud tenant list' to sync.")]
    TenantStore {
        path: String,
        #[source]
        source: BoxedSource,
    },

    #[error(
        "Tenant '{tenant}' not found in local tenant store. Run 'systemprompt cloud tenant list' \
         to sync."
    )]
    TenantNotSynced { tenant: TenantId },

    #[error("{action} failed")]
    Failed {
        action: &'static str,
        #[source]
        source: BoxedSource,
    },
}
