use std::path::Path;

use anyhow::{Context, Result};
use systemprompt_cloud::{CloudPath, TenantStore, get_cloud_paths};
use systemprompt_identifiers::TenantId;
use systemprompt_sync::{SyncApiClient, SyncConfigBuilder, SyncDirection};

use crate::cli_settings::CliConfig;
use crate::commands::cloud::tenant::get_credentials;
use crate::shared::project::ProjectRoot;

pub(crate) async fn build_sync_config(
    tenant_id: &TenantId,
    dry_run: bool,
    _yes: bool,
    _cli_config: &CliConfig,
    _profile_path: &Path,
) -> Result<(systemprompt_sync::SyncConfig, SyncApiClient)> {
    let creds = get_credentials()?;

    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let tenant_store = TenantStore::load_from_path(&tenants_path)
        .context("Tenants not synced. Run 'systemprompt cloud login'")?;

    let tenant = tenant_store.find_tenant(tenant_id.as_str());
    let hostname = tenant.and_then(|t| t.hostname.clone());

    if hostname.is_none() {
        anyhow::bail!("Hostname not configured for tenant.\nRun: systemprompt cloud login");
    }

    let project = ProjectRoot::discover().map_err(|e| anyhow::anyhow!("{}", e))?;
    let local_services_path = project.as_path().join("services");

    let sync_config = SyncConfigBuilder::new(
        tenant_id.clone(),
        &creds.api_url,
        &creds.api_token,
        local_services_path.to_string_lossy(),
    )
    .with_direction(SyncDirection::Pull)
    .with_dry_run(dry_run)
    .with_hostname(hostname.clone())
    .build();

    let api_client =
        SyncApiClient::new(&creds.api_url, &creds.api_token)?.with_direct_sync(hostname);

    Ok((sync_config, api_client))
}
