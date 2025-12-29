use anyhow::{anyhow, bail, Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Password, Select};
use std::fs;
use std::process::Command;
use systemprompt_cloud::constants::checkout::CALLBACK_PORT;
use systemprompt_cloud::constants::regions::AVAILABLE;
use systemprompt_cloud::{
    run_checkout_callback_flow, wait_for_provisioning, CheckoutTemplates, CloudApiClient,
    CloudCredentials, ProjectContext, ProvisioningEventType, StoredTenant, TenantType,
};
use systemprompt_core_logging::CliService;

use crate::cloud::checkout::templates::{ERROR_HTML, SUCCESS_HTML};

use super::docker::{
    generate_postgres_compose, is_port_in_use, nanoid, stop_container_on_port,
    wait_for_postgres_healthy,
};

pub async fn create_local_tenant() -> Result<StoredTenant> {
    CliService::section("Create Local PostgreSQL Tenant");

    let name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Tenant name")
        .default("local".to_string())
        .interact_text()?;

    if name.is_empty() {
        bail!("Tenant name cannot be empty");
    }

    CliService::info("PostgreSQL configuration:");

    let db_user: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Database user")
        .default("systemprompt".to_string())
        .interact_text()?;

    let db_password: String = Password::with_theme(&ColorfulTheme::default())
        .with_prompt("Database password")
        .interact()?;

    if db_password.is_empty() {
        bail!("Password cannot be empty");
    }

    let db_name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Database name")
        .default("systemprompt".to_string())
        .interact_text()?;

    let port: u16 = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Port")
        .default(5432_u16)
        .interact_text()?;

    if is_port_in_use(port) {
        let reset = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Port {} is in use. Stop existing container and reset?",
                port
            ))
            .default(false)
            .interact()?;

        if reset {
            stop_container_on_port(port)?;
        } else {
            bail!("Port {} is in use. Choose a different port.", port);
        }
    }

    let compose_content = generate_postgres_compose(&name, &db_user, &db_password, &db_name, port);
    let ctx = ProjectContext::discover();
    let docker_dir = ctx.docker_dir();
    fs::create_dir_all(&docker_dir).context("Failed to create docker directory")?;

    let compose_path = docker_dir.join(format!("{}.yaml", name));
    fs::write(&compose_path, &compose_content)
        .with_context(|| format!("Failed to write {}", compose_path.display()))?;
    CliService::success(&format!("Created: {}", compose_path.display()));

    CliService::info("Starting PostgreSQL container...");
    let compose_path_str = compose_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid compose path"))?;

    let status = Command::new("docker")
        .args(["compose", "-f", compose_path_str, "up", "-d"])
        .status()
        .context("Failed to execute docker compose. Is Docker running?")?;

    if !status.success() {
        bail!("Failed to start PostgreSQL container. Is Docker running?");
    }

    let spinner = CliService::spinner("Waiting for PostgreSQL to be ready...");
    wait_for_postgres_healthy(&compose_path, 60).await?;
    spinner.finish_and_clear();
    CliService::success("PostgreSQL is ready");

    let database_url = format!(
        "postgres://{}:{}@localhost:{}/{}",
        db_user, db_password, port, db_name
    );

    let id = format!("local_{}", nanoid());
    Ok(StoredTenant::new_local(id, name, database_url))
}

pub async fn create_cloud_tenant(
    creds: &CloudCredentials,
    _default_region: &str,
) -> Result<StoredTenant> {
    CliService::info("Creating cloud tenant via subscription");

    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

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

    let selected_plan = &plans[plan_selection];

    let region_options: Vec<String> = AVAILABLE
        .iter()
        .map(|(code, name)| format!("{} ({})", name, code))
        .collect();

    let region_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a region")
        .items(&region_options)
        .default(0)
        .interact()?;

    let selected_region = AVAILABLE[region_selection].0;

    let redirect_uri = format!("http://127.0.0.1:{}/callback", CALLBACK_PORT);
    let spinner = CliService::spinner("Creating checkout session...");
    let checkout = client
        .create_checkout(
            &selected_plan.paddle_price_id,
            selected_region,
            Some(&redirect_uri),
        )
        .await?;
    spinner.finish_and_clear();

    let templates = CheckoutTemplates {
        success_html: SUCCESS_HTML,
        error_html: ERROR_HTML,
    };

    let result = run_checkout_callback_flow(&client, &checkout.checkout_url, templates).await?;
    CliService::success(&format!(
        "Checkout complete! Tenant ID: {}",
        result.tenant_id
    ));

    let spinner = CliService::spinner("Provisioning cloud infrastructure...");
    let final_event =
        wait_for_provisioning(&client, &result.tenant_id, |event| match event.event_type {
            ProvisioningEventType::VmProvisioningStarted => {
                spinner.set_message("Creating Fly.io app...");
            },
            ProvisioningEventType::VmProvisioned => {
                spinner.set_message("Starting VM...");
            },
            ProvisioningEventType::TenantReady => {
                spinner.set_message("Infrastructure ready!");
            },
            ProvisioningEventType::VmProvisioningProgress => {
                if let Some(msg) = &event.message {
                    spinner.set_message(msg.clone());
                }
            },
            _ => {},
        })
        .await?;
    spinner.finish_and_clear();
    CliService::success("Infrastructure provisioned successfully");

    let spinner = CliService::spinner("Syncing new tenant...");
    let response = client.get_user().await?;
    spinner.finish_and_clear();

    let new_tenant = response
        .tenants
        .iter()
        .find(|t| t.id == result.tenant_id)
        .ok_or_else(|| anyhow!("New tenant not found after checkout"))?;

    Ok(StoredTenant {
        id: new_tenant.id.clone(),
        name: new_tenant.name.clone(),
        tenant_type: TenantType::Cloud,
        app_id: new_tenant.app_id.clone(),
        hostname: new_tenant.hostname.clone().or(final_event.app_url.clone()),
        region: new_tenant.region.clone(),
        database_url: None,
    })
}
