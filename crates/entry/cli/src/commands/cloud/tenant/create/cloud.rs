use anyhow::{Context, Result, anyhow, bail};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Select};
use systemprompt_cloud::constants::api::{DB_PRODUCTION_HOST, DB_SANDBOX_HOST};
use systemprompt_cloud::constants::checkout::CALLBACK_PORT;
use systemprompt_cloud::constants::regions::AVAILABLE;
use systemprompt_cloud::{
    CheckoutTemplates, CloudApiClient, CloudCredentials, StoredTenant, TenantType,
};
use systemprompt_identifiers::TenantId;
use systemprompt_logging::CliService;
use url::Url;

use crate::cloud::deploy::deploy_with_secrets;
use crate::cloud::profile::{collect_api_keys, create_profile_for_tenant};
use crate::cloud::templates::{CHECKOUT_ERROR_HTML, CHECKOUT_SUCCESS_HTML, WAITING_HTML};
use systemprompt_cloud::{run_checkout_callback_flow, wait_for_provisioning};

use super::super::validation::{validate_build_ready, warn_required_secrets};

pub async fn create_cloud_tenant(
    creds: &CloudCredentials,
    _default_region: &str,
) -> Result<StoredTenant> {
    let validation = validate_build_ready().context(
        "Cloud tenant creation requires a built project.\nRun 'just build --release' before \
         creating a cloud tenant.",
    )?;

    CliService::success("Build validation passed");
    CliService::info("Creating cloud tenant via subscription");

    let client = CloudApiClient::new(&creds.api_url, &creds.api_token)?;

    let selected_plan = select_plan(&client).await?;
    let selected_region = select_region()?;

    let redirect_uri = format!("http://127.0.0.1:{}/callback", CALLBACK_PORT);
    let spinner = CliService::spinner("Creating checkout session...");
    let checkout = client
        .create_checkout(&selected_plan, selected_region, Some(&redirect_uri))
        .await?;
    spinner.finish_and_clear();

    let templates = CheckoutTemplates {
        success_html: CHECKOUT_SUCCESS_HTML,
        error_html: CHECKOUT_ERROR_HTML,
        waiting_html: WAITING_HTML,
    };

    let result = run_checkout_callback_flow(&client, &checkout.checkout_url, templates).await?;
    let tenant_id = TenantId::new(&result.tenant_id);
    CliService::success(&format!("Checkout complete! Tenant ID: {}", tenant_id));

    let spinner = CliService::spinner("Waiting for infrastructure provisioning...");
    wait_for_provisioning(&client, tenant_id.as_str(), |event| {
        if let Some(msg) = &event.message {
            CliService::info(msg);
        }
    })
    .await?;
    spinner.finish_and_clear();
    CliService::success("Tenant provisioned successfully");

    let (internal_database_url, sync_token) = fetch_credentials(&client, &tenant_id).await?;

    let (external_db_access, external_database_url) =
        configure_external_access(&client, &tenant_id, &internal_database_url).await?;

    let stored_tenant = build_stored_tenant(
        &client,
        &tenant_id,
        TenantDatabaseConfig {
            external_database_url,
            internal_database_url,
            external_db_access,
            sync_token,
        },
    )
    .await?;

    CliService::section("Profile Setup");
    let profile_name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Profile name")
        .default(stored_tenant.name.clone())
        .interact_text()?;

    CliService::section("API Keys");
    let api_keys = collect_api_keys()?;

    let profile = create_profile_for_tenant(&stored_tenant, &api_keys, &profile_name)?;
    CliService::success(&format!("Profile '{}' created", profile.name));

    if result.needs_deploy {
        CliService::section("Initial Deploy");
        CliService::info("Deploying your code with profile configuration...");
        deploy_with_secrets(&client, &tenant_id, &profile.name).await?;
    }

    warn_required_secrets(&validation.required_secrets);

    Ok(stored_tenant)
}

async fn select_plan(client: &CloudApiClient) -> Result<String> {
    let spinner = CliService::spinner("Fetching available plans...");
    let plans = client.get_plans().await?;
    spinner.finish_and_clear();

    if plans.is_empty() {
        bail!("No plans available. Please contact support.");
    }

    let plan_options: Vec<String> = plans.iter().map(|p| p.name.clone()).collect();

    let plan_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a plan")
        .items(&plan_options)
        .default(0)
        .interact()?;

    Ok(plans[plan_selection].paddle_price_id.clone())
}

fn select_region() -> Result<&'static str> {
    let region_options: Vec<String> = AVAILABLE
        .iter()
        .map(|(code, name)| format!("{} ({})", name, code))
        .collect();

    let region_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a region")
        .items(&region_options)
        .default(0)
        .interact()?;

    Ok(AVAILABLE[region_selection].0)
}

async fn fetch_credentials(
    client: &CloudApiClient,
    tenant_id: &TenantId,
) -> Result<(String, Option<String>)> {
    let spinner = CliService::spinner("Fetching database credentials...");
    let status = client.get_tenant_status(tenant_id.as_str()).await?;
    let secrets_url = status
        .secrets_url
        .ok_or_else(|| anyhow!("Tenant is ready but secrets URL is missing"))?;
    let secrets = client.fetch_secrets(&secrets_url).await?;
    spinner.finish_and_clear();
    CliService::success("Database credentials retrieved");
    Ok((secrets.database_url, secrets.sync_token))
}

async fn configure_external_access(
    client: &CloudApiClient,
    tenant_id: &TenantId,
    internal_database_url: &str,
) -> Result<(bool, Option<String>)> {
    CliService::section("Database Access");
    CliService::info(
        "External database access allows direct PostgreSQL connections from your local machine.",
    );
    CliService::info("This is required for local development workflows.");

    let enable_external = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Enable external database access?")
        .default(true)
        .interact()?;

    if !enable_external {
        CliService::info("External access disabled. Some local features will be limited.");
        return Ok((false, None));
    }

    let spinner = CliService::spinner("Enabling external database access...");
    match client
        .set_external_db_access(tenant_id.as_str(), true)
        .await
    {
        Ok(_) => {
            let external_url = swap_to_external_host(internal_database_url);
            spinner.finish_and_clear();
            CliService::success("External database access enabled");
            print_database_connection_info(&external_url);
            Ok((true, Some(external_url)))
        },
        Err(e) => {
            spinner.finish_and_clear();
            CliService::warning(&format!("Failed to enable external access: {}", e));
            CliService::info("You can enable it later with 'systemprompt cloud tenant edit'");
            Ok((false, None))
        },
    }
}

struct TenantDatabaseConfig {
    external_database_url: Option<String>,
    internal_database_url: String,
    external_db_access: bool,
    sync_token: Option<String>,
}

async fn build_stored_tenant(
    client: &CloudApiClient,
    tenant_id: &TenantId,
    db_config: TenantDatabaseConfig,
) -> Result<StoredTenant> {
    let spinner = CliService::spinner("Syncing new tenant...");
    let response = client.get_user().await?;
    spinner.finish_and_clear();

    let new_tenant = response
        .tenants
        .iter()
        .find(|t| t.id == tenant_id.as_str())
        .ok_or_else(|| anyhow!("New tenant not found after checkout"))?;

    Ok(StoredTenant {
        id: new_tenant.id.clone(),
        name: new_tenant.name.clone(),
        tenant_type: TenantType::Cloud,
        app_id: new_tenant.app_id.clone(),
        hostname: new_tenant.hostname.clone(),
        region: new_tenant.region.clone(),
        database_url: db_config.external_database_url,
        internal_database_url: Some(db_config.internal_database_url),
        external_db_access: db_config.external_db_access,
        sync_token: db_config.sync_token,
        shared_container_db: None,
    })
}

pub fn swap_to_external_host(url: &str) -> String {
    let Ok(parsed) = Url::parse(url) else {
        return url.to_string();
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

fn print_database_connection_info(url: &str) {
    let Ok(parsed) = Url::parse(url) else {
        return;
    };

    let host = parsed.host_str().unwrap_or("unknown");
    let port = parsed.port().unwrap_or(5432);
    let database = parsed.path().trim_start_matches('/');
    let username = parsed.username();
    let password = parsed.password().unwrap_or("********");

    CliService::section("Database Connection");
    CliService::key_value("Host", host);
    CliService::key_value("Port", &port.to_string());
    CliService::key_value("Database", database);
    CliService::key_value("User", username);
    CliService::key_value("Password", password);
    CliService::key_value("SSL", "required");
    CliService::info("");
    CliService::key_value("Connection URL", url);
    CliService::info("");
    CliService::info(&format!(
        "Connect with psql:\n  PGPASSWORD='{}' psql -h {} -p {} -U {} -d {}",
        password, host, port, username, database
    ));
}
