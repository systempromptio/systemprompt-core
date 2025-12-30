use anyhow::{bail, Context, Result};
use systemprompt_cloud::{get_cloud_paths, CloudPath, ProjectContext, TenantStore, TenantType};
use systemprompt_core_logging::CliService;

use super::api_keys::collect_api_keys;
use super::builders::{build_cloud_profile, build_local_profile};
use super::create_setup::{get_cloud_user, handle_local_tenant_setup};
use super::create_tenant::{get_tenants_by_type, select_tenant, select_tenant_type};
use super::templates::{
    generate_display_name, get_services_path, save_dockerfile, save_profile, save_secrets,
};

pub async fn execute(name: &str) -> Result<()> {
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
    let store = TenantStore::load_from_path(&tenants_path).unwrap_or_default();

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

    let secrets_path = profile_dir.join("secrets.json");
    let db_url = tenant
        .database_url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Tenant database URL is required"))?;
    save_secrets(db_url, &api_keys, &secrets_path)?;
    CliService::success(&format!("Created: {}", secrets_path.display()));

    let services_path = get_services_path()?;
    let profile_path = profile_dir.join("profile.yaml");
    let relative_secrets_path = "./secrets.json";

    let built_profile = match tenant.tenant_type {
        TenantType::Local => build_local_profile(
            name,
            &generate_display_name(name),
            Some(tenant.id.clone()),
            relative_secrets_path,
            &services_path,
        )?,
        TenantType::Cloud => {
            let external_url = tenant.hostname.as_ref().map(|h| format!("https://{}", h));
            build_cloud_profile(
                name,
                &generate_display_name(name),
                Some(tenant.id.clone()),
                &services_path,
                external_url.as_deref(),
            )?
        },
    };

    save_profile(&built_profile, &profile_path)?;
    CliService::success(&format!("Created: {}", profile_path.display()));

    let dockerfile_path = ctx.dockerfile();
    if !dockerfile_path.exists() {
        save_dockerfile(&dockerfile_path)?;
        CliService::success(&format!("Created: {}", dockerfile_path.display()));
    }

    match built_profile.validate() {
        Ok(()) => CliService::success("Profile validated"),
        Err(e) => CliService::warning(&format!("Validation warning: {}", e)),
    }

    if tenant.tenant_type == TenantType::Local {
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
