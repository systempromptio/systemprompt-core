use anyhow::{anyhow, bail, Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Password, Select};
use std::fs;
use std::path::Path;
use std::process::Command;
use systemprompt_cloud::constants::checkout::CALLBACK_PORT;
use systemprompt_cloud::constants::regions::AVAILABLE;
use systemprompt_cloud::{
    run_checkout_callback_flow, CheckoutTemplates, CloudApiClient, CloudCredentials,
    ProjectContext, StoredTenant, TenantType,
};
use systemprompt_core_logging::CliService;
use systemprompt_loader::{ConfigLoader, ExtensionLoader};
use systemprompt_models::ServicesConfig;

use crate::cloud::checkout::templates::{ERROR_HTML, SUCCESS_HTML, WAITING_HTML};
use crate::cloud::deploy::deploy_with_secrets;
use crate::cloud::dockerfile;
use crate::cloud::profile::{collect_api_keys, create_profile_for_tenant};
use crate::common::project::ProjectRoot;
use url::Url;

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
        .default(5432u16)
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
    validate_build_ready()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Result of build validation including required secrets.
pub struct BuildValidationResult {
    /// Secrets required for deployment to work.
    pub required_secrets: Vec<String>,
}

fn validate_build_ready() -> Result<BuildValidationResult> {
    let project_root =
        ProjectRoot::discover().context("Must be in a SystemPrompt project directory")?;
    let root = project_root.as_path();

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

    let extension_result = ExtensionLoader::validate(root);
    if !extension_result.missing_binaries.is_empty() {
        let missing_list = extension_result.format_missing_binaries();
        bail!(
            "MCP extension binaries not found:\n\n{}\n\nRun: just build --release",
            missing_list
        );
    }

    dockerfile::check_dockerfile_completeness(root)?;

    let services_path = find_services_config(root)?;
    let services_config = ConfigLoader::load_from_path(&services_path).with_context(|| {
        format!(
            "Failed to load services config: {}",
            services_path.display()
        )
    })?;

    let required_secrets = validate_ai_config(&services_config)?;

    Ok(BuildValidationResult { required_secrets })
}

/// Find the services config file.
fn find_services_config(root: &Path) -> Result<std::path::PathBuf> {
    let paths = [
        root.join("services/config/config.yaml"),
        root.join("core/services/config/config.yaml"),
    ];

    for path in &paths {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    bail!("Services config not found.\n\nExpected at: services/config/config.yaml");
}

/// Validate AI configuration and return required secrets.
fn validate_ai_config(services_config: &ServicesConfig) -> Result<Vec<String>> {
    let ai = &services_config.ai;
    let mut required_secrets = vec![];

    if ai.default_provider.is_empty() {
        bail!(
            "AI config missing default_provider.\n\nSet default_provider in \
             services/ai/config.yaml (e.g., default_provider: \"anthropic\")"
        );
    }

    let provider = ai.providers.get(&ai.default_provider).ok_or_else(|| {
        anyhow!(
            "Default provider '{}' not found in providers.\n\nAdd '{}' to ai.providers in your \
             config.",
            ai.default_provider,
            ai.default_provider
        )
    })?;

    if !provider.enabled {
        bail!(
            "Default provider '{}' is disabled.\n\nSet enabled: true for the '{}' provider.",
            ai.default_provider,
            ai.default_provider
        );
    }

    for (name, prov) in &ai.providers {
        if prov.enabled {
            let secret_key = match name.as_str() {
                "anthropic" => "ANTHROPIC_API_KEY",
                "openai" => "OPENAI_API_KEY",
                "google" => "GOOGLE_API_KEY",
                _ => continue,
            };
            required_secrets.push(secret_key.to_string());
        }
    }

    Ok(required_secrets)
}

/// Display post-deployment secrets warning.
pub fn warn_required_secrets(required_secrets: &[String]) {
    if required_secrets.is_empty() {
        return;
    }

    CliService::warning("Deployment requires API keys to be set via secrets:");
    for secret in required_secrets {
        CliService::info(&format!("  • {}", secret));
    }
    CliService::info("");
    CliService::info("Set secrets with: systemprompt cloud secrets set <KEY> <VALUE>");
    CliService::warning("Your deployment won't work until these secrets are configured.");
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
        success_html: SUCCESS_HTML,
        error_html: ERROR_HTML,
        waiting_html: WAITING_HTML,
    };

    let result = run_checkout_callback_flow(&client, &checkout.checkout_url, templates).await?;
    CliService::success(&format!(
        "Checkout complete! Tenant ID: {}",
        result.tenant_id
    ));

    CliService::success("Tenant provisioned successfully");

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

    let Some(mut database_url) = database_url else {
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

    let external_db_access = if enable_external {
        let spinner = CliService::spinner("Enabling external database access...");
        match client.set_external_db_access(&result.tenant_id, true).await {
            Ok(_) => {
                database_url = swap_to_external_host(&database_url);
                spinner.finish_and_clear();
                CliService::success("External database access enabled");
                print_database_connection_info(&database_url);
                true
            },
            Err(e) => {
                spinner.finish_and_clear();
                CliService::warning(&format!("Failed to enable external access: {}", e));
                CliService::info("You can enable it later with 'systemprompt cloud tenant edit'");
                false
            },
        }
    } else {
        CliService::info("External access disabled. TUI features will be limited.");
        false
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
        database_url: Some(database_url),
        external_db_access,
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

fn swap_to_external_host(url: &str) -> String {
    let Ok(parsed) = Url::parse(url) else {
        return url.to_string();
    };

    let host = parsed.host_str().unwrap_or_default();
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
