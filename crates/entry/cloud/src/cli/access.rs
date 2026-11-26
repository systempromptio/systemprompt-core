//! The cloud member's [`CloudAccess`] for the kernel CLI: the stored cloud
//! credentials, the synced tenant store and the operator token exchange.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_cloud::deploy::provider::tenant_base_url;
use systemprompt_cloud::routing::operator_tenant_token;
use systemprompt_cloud::{CredentialsBootstrap, CredentialsBootstrapError, TenantStore};
use systemprompt_extension::cli::{
    BoxFuture, CloudAccess, CloudAccessError, CloudAccount, TenantRoute,
};
use systemprompt_identifiers::{SessionToken, TenantId};

#[derive(Debug, Clone, Copy, Default)]
pub struct CloudCliAccess;

impl CloudAccess for CloudCliAccess {
    fn account(&self) -> BoxFuture<'_, Result<CloudAccount, CloudAccessError>> {
        Box::pin(async {
            match CredentialsBootstrap::load().await {
                Ok(Some(credentials)) => Ok(CloudAccount::SignedIn {
                    token_expired: credentials.is_token_expired(),
                    user_email: credentials.user_email,
                }),
                Ok(None) => Ok(CloudAccount::LocalOnly {
                    reason: Box::new(CredentialsBootstrapError::NotAvailable),
                }),
                Err(error) if error.is_local_mode_recoverable() => Ok(CloudAccount::LocalOnly {
                    reason: Box::new(error),
                }),
                Err(error) => Err(CloudAccessError::Failed {
                    action: "loading the cloud credentials",
                    source: Box::new(error),
                }),
            }
        })
    }

    fn credentials_present(&self) -> bool {
        matches!(CredentialsBootstrap::read_unvalidated(), Ok(Some(_)))
    }

    fn tenant_route(
        &self,
        tenants_path: &Path,
        tenant: &TenantId,
    ) -> Result<TenantRoute, CloudAccessError> {
        let store = TenantStore::load_from_path(tenants_path).map_err(|source| {
            CloudAccessError::TenantStore {
                path: tenants_path.display().to_string(),
                source: Box::new(source),
            }
        })?;
        let stored =
            store
                .find_tenant(tenant)
                .ok_or_else(|| CloudAccessError::TenantNotSynced {
                    tenant: tenant.clone(),
                })?;
        Ok(TenantRoute {
            hostname: stored.hostname.clone(),
            self_managed: stored.provider.is_self_managed(),
            provider: stored.provider.to_string(),
        })
    }

    fn operator_tenant_token<'a>(
        &'a self,
        hostname: &'a str,
    ) -> BoxFuture<'a, Result<SessionToken, CloudAccessError>> {
        Box::pin(async move {
            operator_tenant_token(hostname)
                .await
                .map_err(|source| CloudAccessError::Failed {
                    action: "the operator token exchange",
                    source: Box::new(source),
                })
        })
    }

    fn tenant_base_url(&self, hostname: &str) -> String {
        tenant_base_url(hostname)
    }
}
