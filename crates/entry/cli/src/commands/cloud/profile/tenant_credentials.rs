//! Refreshes a cloud tenant's masked database credentials before a profile
//! is written from it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use anyhow::Result;
use systemprompt_cloud::{CloudApiClient, StoredTenant, TenantStore, TenantType};
use systemprompt_identifiers::TenantId;
use systemprompt_logging::CliService;
use systemprompt_manifest::Profile;

use crate::commands::cloud::tenant::get_credentials;

struct RefreshedCredentials {
    pub external_database_url: String,
    pub internal_database_url: String,
}

async fn refresh_tenant_credentials(
    client: &CloudApiClient,
    tenant_id: &TenantId,
) -> Result<RefreshedCredentials> {
    let status = client.get_tenant_status(tenant_id).await?;
    let secrets_url = status
        .secrets_url
        .ok_or_else(|| anyhow::anyhow!("No secrets URL available for tenant"))?;
    let secrets = client.fetch_secrets(&secrets_url).await?;
    Ok(RefreshedCredentials {
        external_database_url: secrets.database_url,
        internal_database_url: secrets.internal_database_url,
    })
}

pub async fn ensure_unmasked_credentials(
    tenant: StoredTenant,
    tenants_path: &Path,
) -> Result<StoredTenant> {
    if tenant.tenant_type != TenantType::Cloud {
        return Ok(tenant);
    }

    let external_url = tenant.database_url.as_deref();
    let internal_url = tenant.internal_database_url.as_deref();

    let needs_external = tenant.external_db_access && external_url.is_none();
    let needs_refresh = needs_external
        || external_url.is_some_and(Profile::is_masked_database_url)
        || internal_url.is_none_or(Profile::is_masked_database_url);

    if !needs_refresh {
        return Ok(tenant);
    }

    CliService::info("Fetching database credentials...");
    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, creds.api_token.as_str())?;

    match refresh_tenant_credentials(&client, &tenant.id).await {
        Ok(creds) => {
            let mut updated_tenant = tenant.clone();
            updated_tenant.internal_database_url = Some(creds.internal_database_url);
            if updated_tenant.external_db_access {
                updated_tenant.database_url = Some(creds.external_database_url);
            }

            let mut store = TenantStore::load_from_path(tenants_path)
                .unwrap_or_else(|_| TenantStore::default());
            if let Some(t) = store.tenants.iter_mut().find(|t| t.id == tenant.id) {
                *t = updated_tenant.clone();
                store.save_to_path(tenants_path)?;
            }

            CliService::success("Database credentials retrieved");
            Ok(updated_tenant)
        },
        Err(e) => {
            CliService::warning(&format!("Could not fetch credentials: {}", e));
            CliService::warning(
                "Run 'systemprompt cloud tenant rotate-credentials' to fetch real credentials.",
            );
            Ok(tenant)
        },
    }
}
