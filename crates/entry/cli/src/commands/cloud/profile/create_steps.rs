use anyhow::{Context, Result};
use systemprompt_cloud::{ProfilePath, ProjectContext, StoredTenant, TenantType};
use systemprompt_identifiers::TenantId;
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use super::api_keys::ApiKeys;
use super::builders::{CloudProfileBuilder, LocalProfileBuilder};
use super::templates::{
    DatabaseUrls, get_services_path, save_dockerfile, save_dockerignore, save_entrypoint,
    save_profile, save_secrets, update_ai_config_default_provider,
};

pub struct ProfileFiles {
    pub profile_path: std::path::PathBuf,
}

pub fn write_profile_files(
    ctx: &ProjectContext,
    profile_dir: &std::path::Path,
    name: &str,
    tenant: &StoredTenant,
    api_keys: &ApiKeys,
) -> Result<ProfileFiles> {
    std::fs::create_dir_all(profile_dir)
        .with_context(|| format!("Failed to create directory {}", profile_dir.display()))?;

    std::fs::create_dir_all(ctx.storage_dir()).with_context(|| {
        format!(
            "Failed to create storage directory {}",
            ctx.storage_dir().display()
        )
    })?;

    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);
    let external_url = tenant
        .get_local_database_url()
        .ok_or_else(|| anyhow::anyhow!("Tenant database URL is required"))?;
    let db_urls = DatabaseUrls {
        external: external_url,
        internal: tenant.internal_database_url.as_deref(),
    };
    save_secrets(
        &db_urls,
        api_keys,
        tenant.sync_token.as_deref(),
        &secrets_path,
        tenant.tenant_type == TenantType::Cloud,
    )?;
    CliService::success(&format!("Created: {}", secrets_path.display()));

    update_ai_config_default_provider(api_keys.selected_provider())?;

    let services_path = get_services_path()?;
    let profile_path = ProfilePath::Config.resolve(profile_dir);
    let relative_secrets_path = "./secrets.json";

    let built_profile = build_profile(name, tenant, relative_secrets_path, &services_path);

    save_profile(&built_profile, &profile_path)?;
    CliService::success(&format!("Created: {}", profile_path.display()));

    write_docker_files(ctx, name)?;

    match built_profile.validate() {
        Ok(()) => CliService::success("Profile validated"),
        Err(e) => CliService::warning(&format!("Validation warning: {}", e)),
    }

    Ok(ProfileFiles { profile_path })
}

fn build_profile(
    name: &str,
    tenant: &StoredTenant,
    relative_secrets_path: &str,
    services_path: &str,
) -> Profile {
    match tenant.tenant_type {
        TenantType::Local => LocalProfileBuilder::new(name, relative_secrets_path, services_path)
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
    }
}

fn write_docker_files(ctx: &ProjectContext, name: &str) -> Result<()> {
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

    Ok(())
}
