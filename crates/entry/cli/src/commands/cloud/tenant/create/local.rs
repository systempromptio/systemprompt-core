//! Local tenant creation flows.
//!
//! [`create_local_tenant`] provisions a Docker `PostgreSQL` container owned by
//! that tenant alone; [`create_external_tenant`] registers a user-supplied
//! database after validating the connection. Both then scaffold a local
//! profile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, bail};
use systemprompt_cloud::{DockerCli, ProjectContext, StoredTenant};
use systemprompt_logging::CliService;

use crate::cloud::init::ensure_project_scaffolding;
use crate::cloud::profile::templates::validate_connection;
use crate::cloud::profile::{
    collect_api_keys, create_profile_for_tenant, handle_local_tenant_setup,
};
use crate::interactive::Prompter;

use super::super::docker::{
    TenantContainer, generate_admin_password, is_project_running, nanoid, new_local_tenant_id,
    remove_project, start_project,
};

use super::sanitize_database_name;

pub async fn create_local_tenant(prompter: &dyn Prompter) -> Result<StoredTenant> {
    CliService::section("Create Local PostgreSQL Tenant");

    let name = prompter.input_with_default("Tenant name", "local")?;

    if name.is_empty() {
        bail!("Tenant name cannot be empty");
    }

    let project = format!("{}_{}", sanitize_database_name(&name), nanoid());

    let port: u16 = prompter
        .input_with_default("PostgreSQL port", "5432")?
        .parse()
        .context("PostgreSQL port must be a number")?;

    let docker = DockerCli::new();

    if is_project_running(&docker, &project) {
        bail!("A container for project '{project}' is already running");
    }

    let container = TenantContainer::new(project.clone(), generate_admin_password(), port);

    let spinner = CliService::spinner("Starting PostgreSQL container...");
    let started = start_project(&docker, &container).await;
    spinner.finish_and_clear();

    if let Err(e) = started {
        remove_project(&docker, &project).ok();
        return Err(e);
    }
    CliService::success(&format!("PostgreSQL container '{project}' is ready"));

    let database_url = container.database_url();

    let id = new_local_tenant_id();
    let tenant = StoredTenant::new_local_docker(id, name.clone(), database_url.clone(), project);

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

    let profile_path = ctx.profile_dir(&profile.name).join("profile.yaml");
    handle_local_tenant_setup(prompter, database_url, name, &profile_path).await?;

    Ok(())
}
