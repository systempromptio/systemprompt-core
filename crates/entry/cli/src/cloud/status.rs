//! Cloud status command

use anyhow::Result;
use systemprompt_cloud::{
    get_cloud_paths, CloudApiClient, CloudPath, CredentialsBootstrap, TenantStore,
};
use systemprompt_core_logging::CliService;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

pub async fn execute() -> Result<()> {
    CliService::section("SystemPrompt Cloud Status");

    let cloud_paths = get_cloud_paths()?;
    let mut tenant_id_from_profile: Option<String> = None;

    match ProfileBootstrap::get() {
        Ok(profile) => {
            CliService::key_value(
                "Profile",
                &format!("{} ({})", profile.name, profile.display_name),
            );

            if let Some(cloud) = &profile.cloud {
                CliService::key_value("Cloud enabled", if cloud.enabled { "Yes" } else { "No" });
                CliService::key_value("Credentials path", &cloud.credentials_path);
                CliService::key_value("Validation mode", &format!("{:?}", cloud.validation));

                if let Some(ref tid) = cloud.tenant_id {
                    CliService::key_value("Tenant ID (profile)", tid);
                    tenant_id_from_profile = Some(tid.clone());
                }
            } else {
                CliService::key_value("Cloud config", "Not configured");
            }
        },
        Err(_) => {
            CliService::key_value("Profile", "Not initialized");
        },
    }

    if cloud_paths.exists(CloudPath::Tenants) {
        let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
        if let Ok(store) = TenantStore::load_from_path(&tenants_path) {
            CliService::key_value("Stored tenants", &store.tenants.len().to_string());
            CliService::key_value(
                "Last synced",
                &store.synced_at.format("%Y-%m-%d %H:%M").to_string(),
            );

            if let Some(ref tid) = tenant_id_from_profile {
                if let Some(tenant) = store.find_tenant(tid) {
                    CliService::key_value("Tenant name", &tenant.name);
                    if let Some(ref app_id) = tenant.app_id {
                        CliService::key_value("App ID", app_id);
                    }
                    if let Some(ref hostname) = tenant.hostname {
                        CliService::key_value("Hostname", hostname);
                    }
                    if let Some(ref region) = tenant.region {
                        CliService::key_value("Region", region);
                    }
                } else {
                    CliService::warning(&format!("Tenant {} not found", tid));
                }
            }
        }
    } else {
        CliService::key_value("Tenant store", "Not found (run 'cloud login')");
    }

    match CredentialsBootstrap::get() {
        Ok(Some(creds)) => {
            CliService::key_value("Authenticated", "Yes");
            if let Some(email) = &creds.user_email {
                CliService::key_value("User", email);
            }
            CliService::key_value(
                "Token expired",
                if creds.is_token_expired() {
                    "Yes"
                } else {
                    "No"
                },
            );

            if let Some(ref tenant_id) = tenant_id_from_profile {
                let api_client = CloudApiClient::new(&creds.api_url, &creds.api_token);
                let spinner = CliService::spinner("Fetching tenant status...");
                match api_client.get_tenant_status(tenant_id).await {
                    Ok(status) => {
                        spinner.finish_and_clear();
                        CliService::key_value("Tenant Status", &status.status);
                        if let Some(msg) = status.message {
                            CliService::info(&format!("Message: {}", msg));
                        }
                        if let Some(url) = status.app_url {
                            CliService::key_value("URL", &url);
                        }
                    },
                    Err(e) => {
                        spinner.finish_and_clear();
                        CliService::warning(&format!("Could not fetch tenant status: {}", e));
                    },
                }
            }
        },
        Ok(None) => {
            CliService::key_value("Authenticated", "No (cloud disabled or not configured)");
        },
        Err(_) => {
            CliService::key_value("Authenticated", "No (not initialized)");
            CliService::info("Run 'systemprompt cloud login' to authenticate");
        },
    }

    Ok(())
}
