//! Local tenant creation flows.
//!
//! [`create_local_tenant`] provisions a database inside the shared Docker
//! `PostgreSQL` container (starting it if needed); [`create_external_tenant`]
//! registers a user-supplied database after validating the connection. Both
//! then scaffold a local profile.

use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use systemprompt_cloud::{DockerCli, ProjectContext, StoredTenant};
use systemprompt_logging::CliService;

use crate::cloud::init::ensure_project_scaffolding;
use crate::cloud::profile::templates::validate_connection;
use crate::cloud::profile::{
    collect_api_keys, create_profile_for_tenant, get_cloud_user, handle_local_tenant_setup,
};
use crate::interactive::Prompter;

use super::super::docker::{
    SHARED_ADMIN_USER, SHARED_PORT, SHARED_VOLUME_NAME, SharedContainerConfig, check_volume_exists,
    create_database_for_tenant, ensure_admin_role, generate_admin_password,
    generate_shared_postgres_compose, get_container_password, is_shared_container_running,
    load_shared_config, nanoid, new_local_tenant_id, remove_shared_volume, save_shared_config,
    wait_for_postgres_healthy,
};

use super::sanitize_database_name;

pub async fn create_local_tenant(prompter: &dyn Prompter) -> Result<StoredTenant> {
    CliService::section("Create Local PostgreSQL Tenant");

    let name = prompter.input_with_default("Tenant name", "local")?;

    if name.is_empty() {
        bail!("Tenant name cannot be empty");
    }

    let unique_suffix = nanoid();
    let db_name = format!("{}_{}", sanitize_database_name(&name), unique_suffix);

    let ctx = ProjectContext::discover();
    let docker_dir = ctx.docker_dir();
    fs::create_dir_all(&docker_dir).context("Failed to create docker directory")?;

    let docker = DockerCli::new();

    let shared_config = load_shared_config()?;
    let container_running = is_shared_container_running(&docker);

    let (config, needs_start) =
        resolve_container_state(&docker, shared_config, container_running, prompter)?;

    let compose_path = docker_dir.join("shared.yaml");

    if needs_start {
        start_container(&docker, &config, &compose_path).await?;
    }

    let spinner = CliService::spinner("Verifying admin role...");
    ensure_admin_role(&docker, &config.admin_password)?;
    spinner.finish_and_clear();

    let spinner = CliService::spinner(&format!("Creating database '{}'...", db_name));
    create_database_for_tenant(&docker, &config.admin_password, config.port, &db_name)?;
    spinner.finish_and_clear();
    CliService::success(&format!("Database '{}' created", db_name));

    let database_url = format!(
        "postgres://{}:{}@localhost:{}/{}",
        SHARED_ADMIN_USER, config.admin_password, config.port, db_name
    );

    let id = new_local_tenant_id();
    let tenant =
        StoredTenant::new_local_shared(id, name.clone(), database_url.clone(), db_name.clone());

    let mut updated_config = config;
    updated_config.add_tenant(tenant.id.clone(), db_name);
    save_shared_config(&updated_config)?;

    setup_local_profile(&tenant, &name, &database_url, prompter).await?;

    Ok(tenant)
}

pub async fn create_external_tenant(prompter: &dyn Prompter) -> Result<StoredTenant> {
    CliService::section("Create Local Tenant (External PostgreSQL)");

    let name = prompter.input_with_default("Tenant name", "local")?;

    if name.is_empty() {
        bail!("Tenant name cannot be empty");
    }

    let database_url = prompter.input("PostgreSQL connection URL")?;

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

    let id = new_local_tenant_id();
    let tenant = StoredTenant::new_local(id, name.clone(), database_url.clone());

    setup_local_profile(&tenant, &name, &database_url, prompter).await?;

    Ok(tenant)
}

pub fn resolve_container_state(
    docker: &DockerCli,
    shared_config: Option<SharedContainerConfig>,
    container_running: bool,
    prompter: &dyn Prompter,
) -> Result<(SharedContainerConfig, bool)> {
    match (shared_config, container_running) {
        (Some(config), true) => {
            CliService::info("Using existing shared PostgreSQL container");
            Ok((config, false))
        },
        (Some(config), false) => {
            CliService::info("Shared container config found, restarting container...");
            Ok((config, true))
        },
        (None, true) => {
            CliService::info("Found existing shared PostgreSQL container.");

            let use_existing = prompter.confirm("Use existing container?", true)?;

            if !use_existing {
                bail!(
                    "To create a new container, first stop the existing one:\n  docker stop \
                     systemprompt-postgres-shared && docker rm systemprompt-postgres-shared"
                );
            }

            let spinner = CliService::spinner("Connecting to container...");
            let password = get_container_password(docker)
                .ok_or_else(|| anyhow!("Could not retrieve password from container"))?;
            spinner.finish_and_clear();

            CliService::success("Connected to existing container");
            let config = SharedContainerConfig::new(password, SHARED_PORT);
            Ok((config, false))
        },
        (None, false) => {
            handle_orphaned_volume(docker, prompter)?;

            CliService::info("Creating new shared PostgreSQL container...");
            let password = generate_admin_password();
            let config = SharedContainerConfig::new(password, SHARED_PORT);
            Ok((config, true))
        },
    }
}

pub fn handle_orphaned_volume(docker: &DockerCli, prompter: &dyn Prompter) -> Result<()> {
    if !check_volume_exists(docker) {
        return Ok(());
    }

    CliService::warning("PostgreSQL data volume exists but no container or configuration found.");
    CliService::info(&format!(
        "Volume '{}' contains data from a previous installation.",
        SHARED_VOLUME_NAME
    ));

    let reset = prompter.confirm(
        "Reset volume? (This will delete existing database data)",
        false,
    )?;

    if reset {
        let spinner = CliService::spinner("Removing orphaned volume...");
        remove_shared_volume(docker)?;
        spinner.finish_and_clear();
        CliService::success("Volume removed");
    } else {
        bail!(
            "Cannot create container with orphaned volume.\nEither reset the volume or remove it \
             manually:\n  docker volume rm {}",
            SHARED_VOLUME_NAME
        );
    }

    Ok(())
}

async fn start_container(
    docker: &DockerCli,
    config: &SharedContainerConfig,
    compose_path: &std::path::Path,
) -> Result<()> {
    let compose_content = generate_shared_postgres_compose(&config.admin_password, config.port);
    fs::write(compose_path, &compose_content)
        .with_context(|| format!("Failed to write {}", compose_path.display()))?;
    CliService::success(&format!("Created: {}", compose_path.display()));

    CliService::info("Starting shared PostgreSQL container...");
    let compose_path_str = compose_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid compose path"))?;

    let status = docker
        .status(&["compose", "-f", compose_path_str, "up", "-d"])
        .context("Failed to execute docker compose. Is Docker running?")?;

    if !status.success() {
        bail!("Failed to start PostgreSQL container. Is Docker running?");
    }

    let spinner = CliService::spinner("Waiting for PostgreSQL to be ready...");
    wait_for_postgres_healthy(docker, compose_path, 60).await?;
    spinner.finish_and_clear();
    CliService::success("Shared PostgreSQL container is ready");

    Ok(())
}

async fn setup_local_profile(
    tenant: &StoredTenant,
    name: &str,
    database_url: &str,
    prompter: &dyn Prompter,
) -> Result<()> {
    CliService::section("Profile Setup");
    let profile_name = prompter.input_with_default("Profile name", name)?;

    CliService::section("API Keys");
    let api_keys = collect_api_keys(prompter)?;

    let profile = create_profile_for_tenant(prompter, tenant, &api_keys, &profile_name, None)?;
    CliService::success(&format!("Profile '{}' created", profile.name));

    let ctx = ProjectContext::discover();
    ensure_project_scaffolding(ctx.root())?;

    let cloud_user = get_cloud_user()?;
    let profile_path = ctx.profile_dir(&profile.name).join("profile.yaml");
    handle_local_tenant_setup(prompter, &cloud_user, database_url, name, &profile_path).await?;

    Ok(())
}
