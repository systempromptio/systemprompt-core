//! Interactive OAuth login against systemprompt.io Cloud.
//!
//! Runs the provider-selection prompt and browser OAuth flow, persists the
//! returned credentials and tenant list to the local cloud config paths, and
//! syncs the authenticated admin user into all profiles.

use anyhow::{Result, anyhow};
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use systemprompt_cloud::{
    CloudApiClient, CloudCredentials, CloudPath, OAuthTemplates, TenantInfo, TenantStore,
    UserMeResponse, get_cloud_paths, run_oauth_flow,
};
use systemprompt_logging::CliService;
use systemprompt_models::modules::ApiPaths;

use crate::cli_settings::CliConfig;
use crate::cloud::templates::{AUTH_ERROR_HTML, AUTH_SUCCESS_HTML};
use crate::cloud::types::{
    LoginCustomerInfo, LoginOutput, LoginTenantInfo, LoginUserInfo, TenantPlanInfo,
};
use crate::cloud::{Environment, OAuthProvider};
use crate::shared::CommandOutput;

pub(super) async fn execute(environment: Environment, config: &CliConfig) -> Result<CommandOutput> {
    if !config.is_interactive() {
        return Err(anyhow!(
            "OAuth login requires interactive mode.\n\nAlternatives:\n- Set \
             SYSTEMPROMPT_CLOUD_TOKEN environment variable"
        ));
    }

    let api_url = environment.api_url();

    CliService::section("systemprompt.io Cloud Login");
    CliService::info(&format!("Environment: {:?}", environment));

    let cloud_paths = get_cloud_paths();

    if cloud_paths.exists(CloudPath::Credentials) {
        let creds_path = cloud_paths.resolve(CloudPath::Credentials);
        let existing = CloudCredentials::load_from_path(&creds_path)?;
        CliService::warning(&format!("Already logged in as: {}", existing.user_email));
        CliService::info("Re-authenticating...");
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
        success_html: AUTH_SUCCESS_HTML,
        error_html: AUTH_ERROR_HTML,
    };
    let token = run_oauth_flow(api_url, provider, templates).await?;

    complete_login(api_url, token, config).await
}

pub async fn complete_login(
    api_url: &str,
    token: String,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let cloud_paths = get_cloud_paths();

    let spinner = CliService::spinner("Verifying token...");
    let client = CloudApiClient::new(api_url, &token)?;
    let response = client.get_user().await?;
    spinner.finish_and_clear();

    let creds = CloudCredentials::new(token, api_url.to_owned(), response.user.email.clone());

    let save_path = cloud_paths.resolve(CloudPath::Credentials);
    creds.save_to_path(&save_path)?;
    CliService::key_value("Credentials saved to", &save_path.display().to_string());

    let tenant_store = TenantStore::from_tenant_infos(&response.tenants);
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    tenant_store.save_to_path(&tenants_path)?;
    CliService::key_value("Tenants synced to", &tenants_path.display().to_string());

    CliService::success("Logged in successfully");

    let activity_user_id = systemprompt_identifiers::UserId::new(response.user.id.clone());
    if let Err(e) = client
        .report_activity(ApiPaths::ACTIVITY_EVENT_LOGIN, &activity_user_id)
        .await
    {
        tracing::debug!(error = %e, "Failed to report login activity");
    }

    CliService::section("Syncing Admin User to Profiles");
    if let Some(cloud_user) = crate::cloud::sync::admin_user::CloudUser::from_credentials()? {
        let verbose = config.should_show_verbose();
        let results =
            crate::cloud::sync::admin_user::sync_admin_to_all_profiles(&cloud_user, verbose).await;
        crate::cloud::sync::admin_user::print_sync_results(&results);
    } else {
        CliService::warning("Could not load cloud user for admin sync");
    }

    print_login_result(&response);

    let output = build_login_output(&response, &save_path, &tenants_path);

    Ok(CommandOutput::card_value("Cloud Login", &output).with_skip_render())
}

fn build_login_output(
    response: &UserMeResponse,
    credentials_path: &std::path::Path,
    tenants_path: &std::path::Path,
) -> LoginOutput {
    let user = LoginUserInfo {
        id: response.user.id.as_str().to_owned(),
        email: response.user.email.clone(),
        name: response.user.name.clone(),
    };

    let customer = response
        .customer
        .as_ref()
        .map(|c| LoginCustomerInfo { id: c.id.clone() });

    let tenants: Vec<LoginTenantInfo> = response
        .tenants
        .iter()
        .map(|t| LoginTenantInfo {
            id: t.id.clone(),
            name: t.name.clone(),
            subscription_status: t.subscription_status.map(|s| format!("{s:?}")),
            plan: t.plan.as_ref().map(|p| TenantPlanInfo {
                name: p.name.clone(),
                memory_mb: p.memory_mb,
                volume_gb: p.volume_gb,
            }),
            region: t.region.clone(),
            hostname: t.hostname.clone(),
        })
        .collect();

    LoginOutput {
        user,
        customer,
        tenants,
        credentials_path: credentials_path.display().to_string(),
        tenants_path: tenants_path.display().to_string(),
    }
}

fn print_login_result(response: &UserMeResponse) {
    CliService::section("User");
    CliService::key_value("Email", &response.user.email);
    if let Some(name) = &response.user.name {
        CliService::key_value("Name", name);
    }
    CliService::key_value("ID", response.user.id.as_str());

    if let Some(customer) = &response.customer {
        CliService::section("Customer");
        CliService::key_value("ID", &customer.id);
    }

    print_tenants(&response.tenants);
}

fn print_tenants(tenants: &[TenantInfo]) {
    if tenants.is_empty() {
        CliService::info("No cloud tenants found.");
        CliService::info(
            "Run 'systemprompt cloud tenant create' (or 'just tenant') to create a local tenant.",
        );
        return;
    }

    CliService::section("Available Tenants");
    for tenant in tenants {
        let status_str = tenant
            .subscription_status
            .map_or_else(|| "Unknown".to_owned(), |s| format!("{s:?}"));
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
    CliService::info(
        "Run 'systemprompt cloud tenant create' (or 'just tenant') to add a local tenant,",
    );
    CliService::info("then 'systemprompt cloud profile create <name>' to create a profile.");
}
