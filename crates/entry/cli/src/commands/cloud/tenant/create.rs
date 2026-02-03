use anyhow::{anyhow, bail, Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Select};
use std::fs;
use std::process::Command;
use systemprompt_cloud::constants::checkout::CALLBACK_PORT;
use systemprompt_cloud::constants::regions::AVAILABLE;
use systemprompt_cloud::{
    run_checkout_callback_flow, CheckoutTemplates, CloudApiClient, CloudCredentials,
    ProjectContext, StoredTenant, TenantType,
};
use systemprompt_logging::CliService;
use url::Url;

use crate::cloud::deploy::deploy_with_secrets;
use crate::cloud::profile::{
    collect_api_keys, create_profile_for_tenant, get_cloud_user, handle_local_tenant_setup,
};
use crate::cloud::templates::{CHECKOUT_ERROR_HTML, CHECKOUT_SUCCESS_HTML, WAITING_HTML};

use super::docker::{
    check_volume_exists, create_database_for_tenant, generate_admin_password,
    generate_shared_postgres_compose, get_container_password, is_shared_container_running,
    load_shared_config, nanoid, remove_shared_volume, save_shared_config,
    wait_for_postgres_healthy, SharedContainerConfig, SHARED_ADMIN_USER, SHARED_PORT,
    SHARED_VOLUME_NAME,
};
use super::validation::{validate_build_ready, warn_required_secrets};
use crate::cloud::profile::templates::validate_connection;

pub async fn create_local_tenant() -> Result<StoredTenant> {
    CliService::section("Create Local PostgreSQL Tenant");

    let name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Tenant name")
        .default("local".to_string())
        .interact_text()?;

    if name.is_empty() {
        bail!("Tenant name cannot be empty");
    }

    let unique_suffix = nanoid();
    let db_name = format!("{}_{}", sanitize_database_name(&name), unique_suffix);

    let ctx = ProjectContext::discover();
    let docker_dir = ctx.docker_dir();
    fs::create_dir_all(&docker_dir).context("Failed to create docker directory")?;

    let shared_config = load_shared_config()?;
    let container_running = is_shared_container_running();

    let (config, needs_start) = match (shared_config, container_running) {
        (Some(config), true) => {
            CliService::info("Using existing shared PostgreSQL container");
            (config, false)
        },
        (Some(config), false) => {
            CliService::info("Shared container config found, restarting container...");
            (config, true)
        },
        (None, true) => {
            CliService::info("Found existing shared PostgreSQL container.");

            let use_existing = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Use existing container?")
                .default(true)
                .interact()?;

            if !use_existing {
                bail!(
                    "To create a new container, first stop the existing one:\n  docker stop \
                     systemprompt-postgres-shared && docker rm systemprompt-postgres-shared"
                );
            }

            let spinner = CliService::spinner("Connecting to container...");
            let password = get_container_password()
                .ok_or_else(|| anyhow!("Could not retrieve password from container"))?;
            spinner.finish_and_clear();

            CliService::success("Connected to existing container");
            let config = SharedContainerConfig::new(password, SHARED_PORT);
            (config, false)
        },
        (None, false) => {
            if check_volume_exists() {
                CliService::warning(
                    "PostgreSQL data volume exists but no container or configuration found.",
                );
                CliService::info(&format!(
                    "Volume '{}' contains data from a previous installation.",
                    SHARED_VOLUME_NAME
                ));

                let reset = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Reset volume? (This will delete existing database data)")
                    .default(false)
                    .interact()?;

                if reset {
                    let spinner = CliService::spinner("Removing orphaned volume...");
                    remove_shared_volume()?;
                    spinner.finish_and_clear();
                    CliService::success("Volume removed");
                } else {
                    bail!(
                        "Cannot create container with orphaned volume.\nEither reset the volume \
                         or remove it manually:\n  docker volume rm {}",
                        SHARED_VOLUME_NAME
                    );
                }
            }

            CliService::info("Creating new shared PostgreSQL container...");
            let password = generate_admin_password();
            let config = SharedContainerConfig::new(password, SHARED_PORT);
            (config, true)
        },
    };

    let compose_path = docker_dir.join("shared.yaml");

    if needs_start {
        let compose_content = generate_shared_postgres_compose(&config.admin_password, config.port);
        fs::write(&compose_path, &compose_content)
            .with_context(|| format!("Failed to write {}", compose_path.display()))?;
        CliService::success(&format!("Created: {}", compose_path.display()));

        CliService::info("Starting shared PostgreSQL container...");
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
        CliService::success("Shared PostgreSQL container is ready");
    }

    let spinner = CliService::spinner(&format!("Creating database '{}'...", db_name));
    create_database_for_tenant(&config.admin_password, config.port, &db_name).await?;
    spinner.finish_and_clear();
    CliService::success(&format!("Database '{}' created", db_name));

    let database_url = format!(
        "postgres://{}:{}@localhost:{}/{}",
        SHARED_ADMIN_USER, config.admin_password, config.port, db_name
    );

    let id = format!("local_{}", unique_suffix);
    let tenant =
        StoredTenant::new_local_shared(id, name.clone(), database_url.clone(), db_name.clone());

    let mut updated_config = config;
    updated_config.add_tenant(tenant.id.clone(), db_name);
    save_shared_config(&updated_config)?;

    CliService::section("Profile Setup");
    let profile_name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Profile name")
        .default(name.clone())
        .interact_text()?;

    CliService::section("API Keys");
    let api_keys = collect_api_keys()?;

    let profile = create_profile_for_tenant(&tenant, &api_keys, &profile_name)?;
    CliService::success(&format!("Profile '{}' created", profile.name));

    let cloud_user = get_cloud_user()?;
    let ctx = ProjectContext::discover();
    let profile_path = ctx.profile_dir(&profile.name).join("profile.yaml");
    handle_local_tenant_setup(&cloud_user, &database_url, &name, &profile_path).await?;

    Ok(tenant)
}

pub async fn create_external_tenant() -> Result<StoredTenant> {
    CliService::section("Create Local Tenant (External PostgreSQL)");

    let name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Tenant name")
        .default("local".to_string())
        .interact_text()?;

    if name.is_empty() {
        bail!("Tenant name cannot be empty");
    }

    let database_url: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("PostgreSQL connection URL")
        .interact_text()?;

    if database_url.is_empty() {
        bail!("Database URL cannot be empty");
    }

    let spinner = CliService::spinner("Validating connection...");
    let valid = validate_connection(&database_url).await;
    spinner.finish_and_clear();

    if !valid {
        bail!("Could not connect to database. Check your connection URL and try again.");
    }
    CliService::success("Database connection verified");

    let unique_suffix = nanoid();
    let id = format!("local_{}", unique_suffix);
    let tenant = StoredTenant::new_local(id, name.clone(), database_url.clone());

    CliService::section("Profile Setup");
    let profile_name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Profile name")
        .default(name.clone())
        .interact_text()?;

    CliService::section("API Keys");
    let api_keys = collect_api_keys()?;

    let profile = create_profile_for_tenant(&tenant, &api_keys, &profile_name)?;
    CliService::success(&format!("Profile '{}' created", profile.name));

    let cloud_user = get_cloud_user()?;
    let ctx = ProjectContext::discover();
    let profile_path = ctx.profile_dir(&profile.name).join("profile.yaml");
    handle_local_tenant_setup(&cloud_user, &database_url, &name, &profile_path).await?;

    Ok(tenant)
}

fn sanitize_database_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        "systemprompt".to_string()
    } else if sanitized.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("db_{}", sanitized)
    } else {
        sanitized
    }
}

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
        success_html: CHECKOUT_SUCCESS_HTML,
        error_html: CHECKOUT_ERROR_HTML,
        waiting_html: WAITING_HTML,
    };

    let result = run_checkout_callback_flow(&client, &checkout.checkout_url, templates).await?;
    CliService::success(&format!(
        "Checkout complete! Tenant ID: {}",
        result.tenant_id
    ));

    CliService::success("Tenant provisioned successfully");

    let spinner = CliService::spinner("Fetching database credentials...");
    let (database_url, sync_token) = match client.get_tenant_status(&result.tenant_id).await {
        Ok(status) => {
            if let Some(secrets_url) = status.secrets_url {
                match client.fetch_secrets(&secrets_url).await {
                    Ok(secrets) => (Some(secrets.database_url), secrets.sync_token),
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to fetch secrets");
                        (None, None)
                    },
                }
            } else {
                tracing::warn!("No secrets URL available for tenant {}", result.tenant_id);
                (None, None)
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get tenant status");
            (None, None)
        },
    };
    spinner.finish_and_clear();

    let Some(internal_database_url) = database_url else {
        bail!("Could not retrieve database credentials. Tenant creation incomplete.")
    };
    CliService::success("Database credentials retrieved");

    CliService::section("Database Access");
    CliService::info(
        "External database access allows direct PostgreSQL connections from your local machine.",
    );
    CliService::info("This is required for the TUI and local development workflows.");

    let enable_external = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Enable external database access?")
        .default(true)
        .interact()?;

    let (external_db_access, external_database_url) = if enable_external {
        let spinner = CliService::spinner("Enabling external database access...");
        match client.set_external_db_access(&result.tenant_id, true).await {
            Ok(_) => {
                let external_url = swap_to_external_host(&internal_database_url);
                spinner.finish_and_clear();
                CliService::success("External database access enabled");
                print_database_connection_info(&external_url);
                (true, Some(external_url))
            },
            Err(e) => {
                spinner.finish_and_clear();
                CliService::warning(&format!("Failed to enable external access: {}", e));
                CliService::info("You can enable it later with 'systemprompt cloud tenant edit'");
                (false, None)
            },
        }
    } else {
        CliService::info("External access disabled. TUI features will be limited.");
        (false, None)
    };

    let spinner = CliService::spinner("Syncing new tenant...");
    let response = client.get_user().await?;
    spinner.finish_and_clear();

    let new_tenant = response
        .tenants
        .iter()
        .find(|t| t.id == result.tenant_id)
        .ok_or_else(|| anyhow!("New tenant not found after checkout"))?;

    let stored_tenant = StoredTenant {
        id: new_tenant.id.clone(),
        name: new_tenant.name.clone(),
        tenant_type: TenantType::Cloud,
        app_id: new_tenant.app_id.clone(),
        hostname: new_tenant.hostname.clone(),
        region: new_tenant.region.clone(),
        database_url: external_database_url,
        internal_database_url: Some(internal_database_url),
        external_db_access,
        sync_token,
        shared_container_db: None,
    };

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
        deploy_with_secrets(&client, &result.tenant_id, &profile.name).await?;
    }

    warn_required_secrets(&validation.required_secrets);

    Ok(stored_tenant)
}

pub fn swap_to_external_host(url: &str) -> String {
    let Ok(parsed) = Url::parse(url) else {
        return url.to_string();
    };

    let host = parsed.host_str().unwrap_or("");
    let external_host = if host.contains("sandbox") {
        "db-sandbox.systemprompt.io"
    } else {
        "db.systemprompt.io"
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
