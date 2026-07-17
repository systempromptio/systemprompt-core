//! Cloud tenant provisioning behind a progress seam.
//!
//! [`TenantProvisioningService`] drives the subscription flow end to end:
//! checkout session creation, the browser callback, the provisioning watch
//! ([`wait_for_provisioning`]), database-credential retrieval, optional
//! external database access, and the final tenant-store record. Callers
//! assemble a [`TenantCreatePlan`] up front (all prompting happens before the
//! flow starts) and receive every step boundary as a
//! [`ProvisioningProgressEvent`] for rendering.
//!
//! [`TenantProvisioningService::finalize_tenant`] is the post-checkout half
//! of the flow; it is public so a tenant whose checkout already completed can
//! be resumed without a new payment session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::Url;
use systemprompt_identifiers::{PriceId, TenantId};

use crate::CloudApiClient;
use crate::checkout::{CheckoutTemplates, run_checkout_callback_flow, wait_for_provisioning};
use crate::constants::api::{DB_PRODUCTION_HOST, DB_SANDBOX_HOST};
use crate::error::{CloudError, CloudResult};

use super::{NewCloudTenantParams, StoredTenant};

#[derive(Debug)]
pub struct TenantCreatePlan {
    pub price_id: PriceId,
    pub region: String,
    pub redirect_uri: String,
    pub external_db_access: bool,
    pub templates: CheckoutTemplates,
}

#[derive(Debug)]
pub struct ProvisionedTenant {
    pub tenant: StoredTenant,
    pub needs_deploy: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ProvisioningProgressEvent<'a> {
    CheckoutSessionStarted,
    CheckoutSessionCreated,
    CheckoutComplete { tenant_id: &'a TenantId },
    ProvisioningStarted,
    ProvisioningUpdate { message: &'a str },
    Provisioned,
    CredentialsFetchStarted,
    CredentialsFetched,
    ExternalAccessStarted,
    ExternalAccessEnabled { database_url: &'a str },
    ExternalAccessFailed { error: &'a CloudError },
    TenantSyncStarted,
    TenantSynced,
}

pub trait ProvisioningProgress: Send + Sync {
    fn event(&self, event: &ProvisioningProgressEvent<'_>);
}

#[derive(Debug)]
pub struct TenantProvisioningService<'a> {
    client: &'a CloudApiClient,
}

impl<'a> TenantProvisioningService<'a> {
    #[must_use]
    pub const fn new(client: &'a CloudApiClient) -> Self {
        Self { client }
    }

    pub async fn provision(
        &self,
        plan: &TenantCreatePlan,
        progress: &dyn ProvisioningProgress,
    ) -> CloudResult<ProvisionedTenant> {
        progress.event(&ProvisioningProgressEvent::CheckoutSessionStarted);
        let checkout = self
            .client
            .create_checkout(&plan.price_id, &plan.region, Some(&plan.redirect_uri))
            .await?;
        progress.event(&ProvisioningProgressEvent::CheckoutSessionCreated);

        let result =
            run_checkout_callback_flow(self.client, &checkout.checkout_url, plan.templates).await?;
        progress.event(&ProvisioningProgressEvent::CheckoutComplete {
            tenant_id: &result.tenant_id,
        });

        let tenant = self
            .finalize_tenant(&result.tenant_id, plan.external_db_access, progress)
            .await?;

        Ok(ProvisionedTenant {
            tenant,
            needs_deploy: result.needs_deploy,
        })
    }

    pub async fn finalize_tenant(
        &self,
        tenant_id: &TenantId,
        external_db_access: bool,
        progress: &dyn ProvisioningProgress,
    ) -> CloudResult<StoredTenant> {
        progress.event(&ProvisioningProgressEvent::ProvisioningStarted);
        wait_for_provisioning(self.client, tenant_id, |event| {
            if let Some(message) = &event.message {
                progress.event(&ProvisioningProgressEvent::ProvisioningUpdate { message });
            }
        })
        .await?;
        progress.event(&ProvisioningProgressEvent::Provisioned);

        let internal_database_url = self.fetch_database_url(tenant_id, progress).await?;

        let (enabled, external_database_url) = self
            .configure_external_access(
                tenant_id,
                external_db_access,
                &internal_database_url,
                progress,
            )
            .await?;

        progress.event(&ProvisioningProgressEvent::TenantSyncStarted);
        let response = self.client.get_user().await?;
        progress.event(&ProvisioningProgressEvent::TenantSynced);

        let info = response
            .tenants
            .iter()
            .find(|t| t.id == tenant_id.as_str())
            .ok_or_else(|| CloudError::other("New tenant not found after checkout"))?;

        Ok(StoredTenant::new_cloud(NewCloudTenantParams {
            id: TenantId::new(info.id.clone()),
            name: info.name.clone(),
            app_id: info.app_id.clone(),
            hostname: info.hostname.clone(),
            region: info.region.clone(),
            database_url: external_database_url,
            internal_database_url,
            external_db_access: enabled,
        }))
    }

    async fn fetch_database_url(
        &self,
        tenant_id: &TenantId,
        progress: &dyn ProvisioningProgress,
    ) -> CloudResult<String> {
        progress.event(&ProvisioningProgressEvent::CredentialsFetchStarted);
        let status = self.client.get_tenant_status(tenant_id).await?;
        let secrets_url = status
            .secrets_url
            .ok_or_else(|| CloudError::other("Tenant is ready but secrets URL is missing"))?;
        let secrets = self.client.fetch_secrets(&secrets_url).await?;
        progress.event(&ProvisioningProgressEvent::CredentialsFetched);
        Ok(secrets.database_url)
    }

    async fn configure_external_access(
        &self,
        tenant_id: &TenantId,
        external_db_access: bool,
        internal_database_url: &str,
        progress: &dyn ProvisioningProgress,
    ) -> CloudResult<(bool, Option<String>)> {
        if !external_db_access {
            return Ok((false, None));
        }

        progress.event(&ProvisioningProgressEvent::ExternalAccessStarted);
        match self.client.set_external_db_access(tenant_id, true).await {
            Ok(_) => {
                let external_url = swap_to_external_host(internal_database_url);
                progress.event(&ProvisioningProgressEvent::ExternalAccessEnabled {
                    database_url: &external_url,
                });
                Ok((true, Some(external_url)))
            },
            Err(error) => {
                progress.event(&ProvisioningProgressEvent::ExternalAccessFailed { error: &error });
                Ok((false, None))
            },
        }
    }
}

#[must_use]
pub fn swap_to_external_host(url: &str) -> String {
    let Ok(parsed) = Url::parse(url) else {
        return url.to_owned();
    };

    let host = parsed.host_str().unwrap_or("");
    let external_host = if host.contains("sandbox") {
        DB_SANDBOX_HOST
    } else {
        DB_PRODUCTION_HOST
    };

    url.replace(host, external_host)
        .replace("sslmode=disable", "sslmode=require")
}
