use anyhow::{bail, Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;
use systemprompt_cloud::{
    get_cloud_paths, CloudPath, ProfilePath, ProjectContext, StoredTenant, TenantStore, TenantType,
};
use systemprompt_core_logging::CliService;

use systemprompt_identifiers::TenantId;

use super::api_keys::{collect_api_keys, ApiKeys};
use super::builders::{CloudProfileBuilder, LocalProfileBuilder};
use super::create_setup::{get_cloud_user, handle_local_tenant_setup};
use super::create_tenant::{get_tenants_by_type, select_tenant, select_tenant_type};
use super::templates::{
    get_services_path, save_dockerfile, save_dockerignore, save_entrypoint, save_profile,
    save_secrets, DatabaseUrls,
};
use crate::cli_settings::CliConfig;

pub async fn execute(name: &str, config: &CliConfig) -> Result<()> {
    if !config.is_interactive() {
        return Err(anyhow::anyhow!(
            "Profile creation requires interactive mode.\nUse --tenant-id and --anthropic-key \
             flags in non-interactive mode (not yet implemented)."
        ));
    }
    CliService::section(&format!("Create Profile: {}", name));

    let cloud_user = get_cloud_user()?;
    let ctx = ProjectContext::discover();
    let profile_dir = ctx.profile_dir(name);

    if profile_dir.exists() {
        bail!(
            "Profile '{}' already exists at {}\nUse 'systemprompt cloud profile delete {}' first.",
            name,
            profile_dir.display(),
            name
        );
    }

    std::fs::create_dir_all(ctx.profiles_dir())
        .with_context(|| format!("Failed to create {}", ctx.profiles_dir().display()))?;

    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        CliService::warning(&format!("Failed to load tenant store: {}", e));
        TenantStore::default()
    });

    let tenant_type = select_tenant_type(&store)?;
    let eligible_tenants = get_tenants_by_type(&store, tenant_type);
    let tenant = select_tenant(&eligible_tenants)?;

    if !tenant.has_database_url() {
        bail!(
            "Tenant '{}' does not have a database URL configured.\nFor local tenants, recreate \
             with 'systemprompt cloud tenant create'.",
            tenant.name
        );
    }

    CliService::section("API Keys");
    let api_keys = collect_api_keys()?;

    std::fs::create_dir_all(&profile_dir)
        .with_context(|| format!("Failed to create directory {}", profile_dir.display()))?;

    std::fs::create_dir_all(ctx.storage_dir()).with_context(|| {
        format!(
            "Failed to create storage directory {}",
            ctx.storage_dir().display()
        )
    })?;

    let secrets_path = ProfilePath::Secrets.resolve(&profile_dir);
    let external_url = tenant
        .get_local_database_url()
        .ok_or_else(|| anyhow::anyhow!("Tenant database URL is required"))?;
    let db_urls = DatabaseUrls {
        external: external_url,
        internal: tenant.internal_database_url.as_deref(),
    };
    save_secrets(&db_urls, &api_keys, &secrets_path)?;
    CliService::success(&format!("Created: {}", secrets_path.display()));

    let services_path = get_services_path()?;
    let profile_path = ProfilePath::Config.resolve(&profile_dir);
    let relative_secrets_path = "./secrets.json";

    let built_profile = match tenant.tenant_type {
        TenantType::Local => LocalProfileBuilder::new(name, relative_secrets_path, &services_path)
            .with_tenant_id(TenantId::new(&tenant.id))
            .build(),
        TenantType::Cloud => {
            let mut builder = CloudProfileBuilder::new(name)
                .with_tenant_id(TenantId::new(&tenant.id))
                .with_external_db_access(tenant.external_db_access)
                .with_secrets_path(relative_secrets_path);
            if let Some(hostname) = &tenant.hostname {
                builder = builder.with_external_url(format!("https://{}", hostname));
            }
            builder.build()
        },
    };

    save_profile(&built_profile, &profile_path)?;
    CliService::success(&format!("Created: {}", profile_path.display()));

    let docker_dir = ctx.profile_docker_dir(name);
    std::fs::create_dir_all(&docker_dir)
        .with_context(|| format!("Failed to create docker directory {}", docker_dir.display()))?;

    let dockerfile_path = ctx.profile_dockerfile(name);
    save_dockerfile(&dockerfile_path, name, ctx.root())?;
    CliService::success(&format!("Created: {}", dockerfile_path.display()));

    let entrypoint_path = ctx.profile_entrypoint(name);
    save_entrypoint(&entrypoint_path)?;
    CliService::success(&format!("Created: {}", entrypoint_path.display()));

    let dockerignore_path = ctx.profile_dockerignore(name);
    save_dockerignore(&dockerignore_path)?;
    CliService::success(&format!("Created: {}", dockerignore_path.display()));

    match built_profile.validate() {
        Ok(()) => CliService::success("Profile validated"),
        Err(e) => CliService::warning(&format!("Validation warning: {}", e)),
    }

    if tenant.tenant_type == TenantType::Local {
        let db_url = tenant
            .get_local_database_url()
            .ok_or_else(|| anyhow::anyhow!("Tenant database URL is required"))?;
        handle_local_tenant_setup(&cloud_user, db_url, &tenant.name, &profile_path).await?;
    }

    CliService::section("Next Steps");
    CliService::info(&format!(
        "  export SYSTEMPROMPT_PROFILE={}",
        profile_path.display()
    ));

    match tenant.tenant_type {
        TenantType::Local => CliService::info("  just start"),
        TenantType::Cloud => CliService::info("  just deploy"),
    }

    Ok(())
}

#[derive(Debug)]
pub struct CreatedProfile {
    pub name: String,
}

pub fn create_profile_for_tenant(
    tenant: &StoredTenant,
    api_keys: &ApiKeys,
    profile_name: &str,
) -> Result<CreatedProfile> {
    let ctx = ProjectContext::discover();
    let mut name = profile_name.to_string();

    loop {
        let profile_dir = ctx.profile_dir(&name);
        if !profile_dir.exists() {
            break;
        }

        CliService::warning(&format!(
            "Profile '{}' already exists at {}",
            name,
            profile_dir.display()
        ));

        name = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter a different profile name")
            .interact_text()?;
    }

    let profile_dir = ctx.profile_dir(&name);

    std::fs::create_dir_all(ctx.profiles_dir())
        .with_context(|| format!("Failed to create {}", ctx.profiles_dir().display()))?;

    std::fs::create_dir_all(&profile_dir)
        .with_context(|| format!("Failed to create directory {}", profile_dir.display()))?;

    std::fs::create_dir_all(ctx.storage_dir()).with_context(|| {
        format!(
            "Failed to create storage directory {}",
            ctx.storage_dir().display()
        )
    })?;

    let secrets_path = ProfilePath::Secrets.resolve(&profile_dir);
    let local_db_url = tenant
        .get_local_database_url()
        .ok_or_else(|| anyhow::anyhow!("Tenant database URL is required"))?;
    let db_urls = DatabaseUrls {
        external: local_db_url,
        internal: tenant.internal_database_url.as_deref(),
    };
    save_secrets(&db_urls, api_keys, &secrets_path)?;
    CliService::success(&format!("Created: {}", secrets_path.display()));

    let profile_path = ProfilePath::Config.resolve(&profile_dir);

    let mut builder = CloudProfileBuilder::new(&name)
        .with_tenant_id(TenantId::new(&tenant.id))
        .with_external_db_access(tenant.external_db_access)
        .with_secrets_path("./secrets.json");
    if let Some(hostname) = &tenant.hostname {
        builder = builder.with_external_url(format!("https://{}", hostname));
    }
    let built_profile = builder.build();

    save_profile(&built_profile, &profile_path)?;
    CliService::success(&format!("Created: {}", profile_path.display()));

    let docker_dir = ctx.profile_docker_dir(&name);
    std::fs::create_dir_all(&docker_dir)
        .with_context(|| format!("Failed to create docker directory {}", docker_dir.display()))?;

    let dockerfile_path = ctx.profile_dockerfile(&name);
    save_dockerfile(&dockerfile_path, &name, ctx.root())?;
    CliService::success(&format!("Created: {}", dockerfile_path.display()));

    let entrypoint_path = ctx.profile_entrypoint(&name);
    save_entrypoint(&entrypoint_path)?;
    CliService::success(&format!("Created: {}", entrypoint_path.display()));

    let dockerignore_path = ctx.profile_dockerignore(&name);
    save_dockerignore(&dockerignore_path)?;
    CliService::success(&format!("Created: {}", dockerignore_path.display()));

    match built_profile.validate() {
        Ok(()) => CliService::success("Profile validated"),
        Err(e) => CliService::warning(&format!("Validation warning: {}", e)),
    }

    Ok(CreatedProfile { name })
}
