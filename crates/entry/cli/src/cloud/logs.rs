//! Cloud logs command

use anyhow::{bail, Result};
use systemprompt_cloud::{get_cloud_paths, CloudApiClient, CloudPath, TenantStore, TenantType};
use systemprompt_core_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

use super::tenant_ops::get_credentials;

pub async fn execute(tenant_id: Option<String>, lines: u32) -> Result<()> {
    CliService::section("Tenant Logs");

    let resolved_tenant_id = resolve_tenant_id(tenant_id)?;

    // Verify this is a cloud tenant
    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    if let Ok(store) = TenantStore::load_from_path(&tenants_path) {
        if let Some(tenant) = store.find_tenant(&resolved_tenant_id) {
            if tenant.tenant_type != TenantType::Cloud {
                bail!("Logs are only available for cloud tenants");
            }
        }
    }

    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    let spinner = CliService::spinner(&format!(
        "Fetching logs for tenant {}...",
        resolved_tenant_id
    ));
    match client.get_logs(&resolved_tenant_id, lines).await {
        Ok(logs) => {
            spinner.finish_and_clear();

            if logs.is_empty() {
                CliService::info("No logs available");
            } else {
                CliService::info(&format!("Showing last {} log entries:", logs.len()));
                println!();

                for entry in logs {
                    let level_indicator = entry
                        .level
                        .as_ref()
                        .map(|l| format!("[{}]", l.to_uppercase()))
                        .unwrap_or_default();

                    if level_indicator.is_empty() {
                        println!("{} {}", entry.timestamp, entry.message);
                    } else {
                        println!("{} {} {}", entry.timestamp, level_indicator, entry.message);
                    }
                }
            }
        },
        Err(e) => {
            spinner.finish_and_clear();
            bail!("Failed to fetch logs: {}", e);
        },
    }

    Ok(())
}

fn resolve_tenant_id(tenant_id: Option<String>) -> Result<String> {
    if let Some(id) = tenant_id {
        return Ok(id);
    }

    // Try to get from profile
    if let Ok(profile) = ProfileBootstrap::get() {
        if let Some(cloud) = &profile.cloud {
            if let Some(ref tid) = cloud.tenant_id {
                return Ok(tid.clone());
            }
        }
    }

    bail!("No tenant specified. Use --tenant or configure a tenant in your profile.")
}
