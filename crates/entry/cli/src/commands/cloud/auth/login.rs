use anyhow::{anyhow, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_cloud::{
    get_cloud_paths, run_oauth_flow, CloudApiClient, CloudCredentials, CloudPath, OAuthTemplates,
    TenantStore,
};
use systemprompt_core_logging::CliService;

use crate::cli_settings::CliConfig;
use crate::cloud::oauth::{ERROR_HTML, SUCCESS_HTML};
use crate::cloud::{Environment, OAuthProvider};

pub async fn execute(environment: Environment, config: &CliConfig) -> Result<()> {
    if !config.is_interactive() {
        return Err(anyhow!(
            "OAuth login requires interactive mode.\n\n\
             Alternatives:\n\
             - Set SYSTEMPROMPT_CLOUD_TOKEN environment variable"
        ));
    }

    let api_url = environment.api_url();

    CliService::section("SystemPrompt Cloud Login");
    CliService::info(&format!("Environment: {:?}", environment));

    let cloud_paths = get_cloud_paths()?;

    if cloud_paths.exists(CloudPath::Credentials) {
        let creds_path = cloud_paths.resolve(CloudPath::Credentials);
        let existing = CloudCredentials::load_from_path(&creds_path)?;
        if let Some(email) = &existing.user_email {
            CliService::warning(&format!("Already logged in as: {email}"));
            CliService::info("Re-authenticating...");
        }
    }

    let providers = [OAuthProvider::Github, OAuthProvider::Google];
    let provider_names: Vec<&str> = providers.iter().map(OAuthProvider::display_name).collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select authentication provider")
        .items(&provider_names)
        .default(0)
        .interact()?;

    let provider = providers[selection];

    let templates = OAuthTemplates {
        success_html: SUCCESS_HTML,
        error_html: ERROR_HTML,
    };
    let token = run_oauth_flow(api_url, provider, templates).await?;

    let spinner = CliService::spinner("Verifying token...");
    let client = CloudApiClient::new(api_url, &token);
    let response = client.get_user().await?;
    spinner.finish_and_clear();

    let creds = CloudCredentials::new(
        token,
        api_url.to_string(),
        Some(response.user.email.clone()),
    );

    let save_path = cloud_paths.resolve(CloudPath::Credentials);
    creds.save_to_path(&save_path)?;
    CliService::key_value("Credentials saved to", &save_path.display().to_string());

    let tenant_store = TenantStore::from_tenant_infos(&response.tenants);
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    tenant_store.save_to_path(&tenants_path)?;
    CliService::key_value("Tenants synced to", &tenants_path.display().to_string());

    CliService::success("Logged in successfully");

    CliService::section("Syncing Admin User to Profiles");
    if let Some(cloud_user) = crate::cloud::sync::admin_user::CloudUser::from_credentials()? {
        let results = crate::cloud::sync::admin_user::sync_admin_to_all_profiles(&cloud_user).await;
        crate::cloud::sync::admin_user::print_sync_results(&results);
    } else {
        CliService::warning("Could not load cloud user for admin sync");
    }

    CliService::section("User");
    CliService::key_value("Email", &response.user.email);
    if let Some(name) = &response.user.name {
        CliService::key_value("Name", name);
    }
    CliService::key_value("ID", &response.user.id);

    if let Some(customer) = &response.customer {
        CliService::section("Customer");
        CliService::key_value("ID", &customer.id);
    }

    if response.tenants.is_empty() {
        CliService::info("No cloud tenants found.");
        CliService::info("Run 'systemprompt cloud tenant create' to create a local tenant.");
    } else {
        CliService::section("Available Tenants");
        for tenant in &response.tenants {
            let status_str = tenant
                .subscription_status
                .map_or_else(|| "Unknown".to_string(), |s| format!("{s:?}"));
            CliService::key_value(&tenant.name, &status_str);
            if let Some(plan) = &tenant.plan {
                CliService::info(&format!(
                    "  Plan: {} ({}MB RAM, {}GB storage)",
                    plan.name, plan.memory_mb, plan.volume_gb
                ));
            }
            if let Some(region) = &tenant.region {
                CliService::info(&format!("  Region: {region}"));
            }
            if let Some(hostname) = &tenant.hostname {
                CliService::info(&format!("  URL: https://{hostname}"));
            }
        }
        CliService::info("");
        CliService::info("Run 'systemprompt cloud tenant create' to add a local tenant,");
        CliService::info("then 'systemprompt cloud profile create <name>' to create a profile.");
    }

    Ok(())
}
