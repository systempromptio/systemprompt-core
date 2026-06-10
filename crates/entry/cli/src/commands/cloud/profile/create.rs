//! `cloud profile create`: scaffold a profile for a chosen tenant.
//!
//! Selects or resolves the tenant, collects API keys, writes the secrets,
//! profile, and Docker artifacts, then validates the result and runs
//! local-tenant setup where applicable.

use std::path::Path;

use anyhow::{Context, Result, bail};
use systemprompt_cloud::{
    CloudPath, ProfilePath, ProjectContext, StoredTenant, TenantStore, TenantType, get_cloud_paths,
};
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use systemprompt_identifiers::TenantId;

use super::CreateArgs;
use super::api_keys::{ApiKeys, collect_api_keys};
use super::create_setup::{get_cloud_user, handle_local_tenant_setup};
use super::create_tenant::{get_tenants_by_type, select_tenant, select_tenant_type};
use super::profile_steps::{
    ensure_profile_dirs, ensure_unmasked_credentials, report_profile_validation,
    resolve_tenant_from_args, write_docker_assets, write_profile_secrets,
};
use super::templates::{get_services_path, save_profile, update_ai_config_default_provider};
use crate::cli_settings::CliConfig;
use systemprompt_cloud::profile_authoring::{CloudProfileBuilder, LocalProfileBuilder};

pub use super::profile_steps::{CreatedProfile, create_profile_for_tenant};

pub(super) async fn execute(args: &CreateArgs, config: &CliConfig) -> Result<()> {
    let name = &args.name;
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

    let cloud_paths = get_cloud_paths();
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        CliService::warning(&format!("Failed to load tenant store: {}", e));
        TenantStore::default()
    });

    let (tenant, api_keys) = select_tenant_and_keys(args, config, &store)?;
    let tenant = ensure_unmasked_credentials(tenant, &tenants_path).await?;

    ensure_profile_dirs(&ctx, &profile_dir)?;
    write_profile_secrets(&tenant, &api_keys, &profile_dir)?;
    update_ai_config_default_provider(api_keys.selected_provider())?;

    let profile_path = ProfilePath::Config.resolve(&profile_dir);
    let built_profile = build_tenant_profile(name, &tenant)?;

    save_profile(&built_profile, &profile_path)?;
    CliService::success(&format!("Created: {}", profile_path.display()));

    write_docker_assets(&ctx, name)?;
    report_profile_validation(&built_profile);

    if tenant.tenant_type == TenantType::Local {
        let db_url = tenant
            .get_local_database_url()
            .ok_or_else(|| anyhow::anyhow!("Tenant database URL is required"))?;
        handle_local_tenant_setup(&cloud_user, db_url, &tenant.name, &profile_path).await?;
    }

    render_next_steps(&tenant, &profile_path);

    Ok(())
}

fn select_tenant_and_keys(
    args: &CreateArgs,
    config: &CliConfig,
    store: &TenantStore,
) -> Result<(StoredTenant, ApiKeys)> {
    if config.is_interactive() && args.tenant.is_none() {
        let tenant_type = select_tenant_type(store)?;
        let eligible_tenants = get_tenants_by_type(store, tenant_type);
        let tenant = select_tenant(&eligible_tenants)?;
        ensure_tenant_database(&tenant)?;

        CliService::section("API Keys");
        let api_keys = collect_api_keys()?;
        Ok((tenant, api_keys))
    } else {
        let tenant = resolve_tenant_from_args(args, store)?;
        ensure_tenant_database(&tenant)?;

        let api_keys = ApiKeys::from_options(
            args.gemini_key.clone(),
            args.anthropic_key.clone(),
            args.openai_key.clone(),
        )?;
        Ok((tenant, api_keys))
    }
}

fn ensure_tenant_database(tenant: &StoredTenant) -> Result<()> {
    if !tenant.has_database_url() {
        bail!(
            "Tenant '{}' does not have a database URL configured.\nFor local tenants, recreate \
             with 'systemprompt cloud tenant create'.",
            tenant.name
        );
    }
    Ok(())
}

fn build_tenant_profile(name: &str, tenant: &StoredTenant) -> Result<Profile> {
    let services_path = get_services_path()?;
    let relative_secrets_path = "./secrets.json";

    Ok(match tenant.tenant_type {
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
    })
}

fn render_next_steps(tenant: &StoredTenant, profile_path: &Path) {
    CliService::section("Next Steps");
    CliService::info(&format!(
        "  export SYSTEMPROMPT_PROFILE={}",
        profile_path.display()
    ));

    match tenant.tenant_type {
        TenantType::Local => CliService::info("  just start"),
        TenantType::Cloud => CliService::info("  just deploy"),
    }
}
