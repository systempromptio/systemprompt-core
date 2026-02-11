use anyhow::Result;
use chrono::Duration;
use systemprompt_cloud::{
    get_cloud_paths, CloudApiClient, CloudCredentials, CloudPath, ProfilePath, ProjectContext,
};
use systemprompt_logging::CliService;

use crate::cli_settings::CliConfig;
use crate::cloud::types::WhoamiOutput;
use crate::shared::CommandResult;

pub async fn execute(config: &CliConfig) -> Result<CommandResult<WhoamiOutput>> {
    let cloud_paths = get_cloud_paths()?;

    if !cloud_paths.exists(CloudPath::Credentials) {
        if !config.is_json_output() {
            CliService::section("systemprompt.io Cloud Identity");
            CliService::warning("Not logged in");
            CliService::info("Run 'systemprompt cloud auth login' to authenticate.");
        }

        let output = WhoamiOutput {
            user_email: String::new(),
            api_url: String::new(),
            token_status: "Not logged in".to_string(),
            authenticated_at: chrono::Utc::now(),
            local_profiles: count_local_profiles(),
            cloud_tenants: 0,
        };

        return Ok(CommandResult::card(output).with_title("Cloud Identity"));
    }

    let creds_path = cloud_paths.resolve(CloudPath::Credentials);
    let creds = CloudCredentials::load_from_path(&creds_path)?;

    let token_status = if creds.is_token_expired() {
        "Expired".to_string()
    } else if creds.expires_within(Duration::hours(1)) {
        "Expires soon (within 1 hour)".to_string()
    } else {
        "Valid".to_string()
    };

    let cloud_count = fetch_cloud_tenant_count(&creds).await;
    let local_count = count_local_profiles();

    let output = WhoamiOutput {
        user_email: creds.user_email.clone(),
        api_url: creds.api_url.clone(),
        token_status: token_status.clone(),
        authenticated_at: creds.authenticated_at,
        local_profiles: local_count,
        cloud_tenants: cloud_count,
    };

    if !config.is_json_output() {
        CliService::section("systemprompt.io Cloud Identity");
        CliService::key_value("User", &creds.user_email);
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

        CliService::section("Tenants");
        CliService::key_value("Local profiles", &local_count.to_string());
        CliService::key_value("Cloud tenants", &cloud_count.to_string());
    }

    Ok(CommandResult::card(output).with_title("Cloud Identity"))
}

async fn fetch_cloud_tenant_count(creds: &CloudCredentials) -> usize {
    if creds.is_token_expired() {
        return 0;
    }

    let Ok(client) = CloudApiClient::new(&creds.api_url, &creds.api_token) else {
        return 0;
    };
    client
        .list_tenants()
        .await
        .map(|tenants| tenants.len())
        .unwrap_or(0)
}

fn count_local_profiles() -> usize {
    let ctx = ProjectContext::discover();
    let profiles_dir = ctx.profiles_dir();

    if !profiles_dir.exists() {
        return 0;
    }

    std::fs::read_dir(&profiles_dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|e| e.path().is_dir() && ProfilePath::Config.resolve(&e.path()).exists())
                .count()
        })
        .unwrap_or(0)
}
