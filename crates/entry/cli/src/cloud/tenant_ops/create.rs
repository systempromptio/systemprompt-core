use anyhow::{anyhow, bail, Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Password, Select};
use std::fs;
use std::process::Command;
use systemprompt_cloud::constants::checkout::CALLBACK_PORT;
use systemprompt_cloud::constants::regions::AVAILABLE;
use systemprompt_cloud::{
    run_checkout_callback_flow, CheckoutTemplates, CloudApiClient, CloudCredentials,
    ProjectContext, StoredTenant, TenantType,
};
use systemprompt_core_logging::CliService;

use crate::cloud::checkout::templates::{ERROR_HTML, SUCCESS_HTML, WAITING_HTML};
use crate::cloud::deploy::deploy_initial;
use crate::common::project::ProjectRoot;

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

pub fn check_build_ready() -> Result<(), String> {
    validate_build_ready().map_err(|e| e.to_string())
}

fn validate_build_ready() -> Result<()> {
    let project_root =
        ProjectRoot::discover().context("Must be in a SystemPrompt project directory")?;
    let root = project_root.as_path();

    let dockerfile = root.join(".systemprompt/Dockerfile");
    if !dockerfile.exists() {
        bail!(
            "Dockerfile not found: {}\n\nCloud tenant creation requires a Dockerfile.\nCreate one \
             at .systemprompt/Dockerfile",
            dockerfile.display()
        );
    }

    let binary_paths = [
        root.join("core/target/release/systemprompt"),
        root.join("target/release/systemprompt"),
    ];
    if !binary_paths.iter().any(|p| p.exists()) {
        bail!(
            "Release binary not found.\n\nCloud tenant creation requires a built binary.\nRun: \
             just build --release\nOr:  cargo build --release --bin systemprompt"
        );
    }

    let web_dist_paths = [root.join("core/web/dist"), root.join("web/dist")];
    let web_dist = web_dist_paths.iter().find(|p| p.exists());
    match web_dist {
        None => bail!(
            "Web dist not found.\n\nCloud tenant creation requires built web assets.\nRun: just \
             build --release\nOr:  cd core/web && npm run build"
        ),
        Some(dist_path) if !dist_path.join("index.html").exists() => bail!(
            "Web dist missing index.html: {}\n\nRun: just build --release",
            dist_path.display()
        ),
        Some(_) => {},
    }

    Ok(())
}

pub async fn create_cloud_tenant(
    creds: &CloudCredentials,
    _default_region: &str,
) -> Result<StoredTenant> {
    validate_build_ready().context(
        "Cloud tenant creation requires a built project.\nRun 'just build --release' before \
         creating a cloud tenant.",
    )?;

    CliService::success("Build validation passed");
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
        waiting_html: WAITING_HTML,
    };

    let result = run_checkout_callback_flow(&client, &checkout.checkout_url, templates).await?;
    CliService::success(&format!(
        "Checkout complete! Tenant ID: {}",
        result.tenant_id
    ));

    if result.needs_deploy {
        CliService::info("Infrastructure ready, deploying your code...");
        deploy_initial(&client, &result.tenant_id).await?;
    }

    CliService::success("Tenant provisioned successfully");

    let spinner = CliService::spinner("Syncing new tenant...");
    let response = client.get_user().await?;
    spinner.finish_and_clear();

    let new_tenant = response
        .tenants
        .iter()
        .find(|t| t.id == result.tenant_id)
        .ok_or_else(|| anyhow!("New tenant not found after checkout"))?;

    // Fetch database credentials from the one-time secrets endpoint
    let spinner = CliService::spinner("Fetching database credentials...");
    let database_url = match client.get_tenant_status(&result.tenant_id).await {
        Ok(status) => {
            if let Some(secrets_url) = status.secrets_url {
                match client.fetch_secrets(&secrets_url).await {
                    Ok(secrets) => Some(secrets.database_url),
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to fetch secrets");
                        None
                    },
                }
            } else {
                tracing::warn!("No secrets URL available for tenant {}", result.tenant_id);
                None
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get tenant status");
            None
        },
    };
    spinner.finish_and_clear();

    if database_url.is_some() {
        CliService::success("Database credentials retrieved");
    } else {
        CliService::warning(
            "Could not retrieve database credentials. You may need to recreate the tenant.",
        );
    }

    Ok(StoredTenant {
        id: new_tenant.id.clone(),
        name: new_tenant.name.clone(),
        tenant_type: TenantType::Cloud,
        app_id: new_tenant.app_id.clone(),
        hostname: new_tenant.hostname.clone(),
        region: new_tenant.region.clone(),
        database_url,
    })
}
