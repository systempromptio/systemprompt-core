//! Cloud whoami command

use anyhow::Result;
use chrono::Duration;
use systemprompt_cloud::{get_cloud_paths, CloudCredentials, CloudPath, TenantStore, TenantType};
use systemprompt_core_logging::CliService;

pub async fn execute() -> Result<()> {
    CliService::section("SystemPrompt Cloud Identity");

    let cloud_paths = get_cloud_paths()?;

    if !cloud_paths.exists(CloudPath::Credentials) {
        CliService::warning("Not logged in");
        CliService::info("Run 'systemprompt cloud auth login' to authenticate.");
        return Ok(());
    }

    let creds_path = cloud_paths.resolve(CloudPath::Credentials);
    let creds = CloudCredentials::load_from_path(&creds_path)?;

    if let Some(email) = &creds.user_email {
        CliService::key_value("User", email);
    } else {
        CliService::key_value("User", "(unknown)");
    }

    CliService::key_value("API URL", &creds.api_url);

    if creds.is_token_expired() {
        CliService::warning("Token status: Expired");
        CliService::info("Run 'systemprompt cloud auth login' to refresh.");
    } else if creds.expires_within(Duration::hours(1)) {
        CliService::warning("Token status: Expires soon (within 1 hour)");
    } else {
        CliService::key_value("Token status", "Valid");
    }

    CliService::key_value(
        "Authenticated at",
        &creds
            .authenticated_at
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
    );

    if cloud_paths.exists(CloudPath::Tenants) {
        let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
        if let Ok(store) = TenantStore::load_from_path(&tenants_path) {
            let local_count = store
                .tenants
                .iter()
                .filter(|t| t.tenant_type == TenantType::Local)
                .count();
            let cloud_count = store.tenants.len() - local_count;

            CliService::section("Tenants");
            CliService::key_value("Local", &local_count.to_string());
            CliService::key_value("Cloud", &cloud_count.to_string());
            CliService::key_value(
                "Last synced",
                &store.synced_at.format("%Y-%m-%d %H:%M").to_string(),
            );
        }
    }

    Ok(())
}
