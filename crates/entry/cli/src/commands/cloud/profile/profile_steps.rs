//! Shared steps for building a profile from a stored tenant.
//!
//! Provides [`create_profile_for_tenant`] and the helpers that write a
//! tenant's profile, secrets and Docker assets and resolve a tenant from CLI
//! args.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use anyhow::{Context, Result, bail};
use systemprompt_cloud::{ProfilePath, ProjectContext, StoredTenant, TenantStore, TenantType};
use systemprompt_logging::CliService;
use systemprompt_manifest::Profile;

use systemprompt_identifiers::ProfileName;


use systemprompt_manifest::profile::TrustedIssuer;

use super::api_keys::ApiKeys;
use super::templates::{
    DatabaseUrls, existing_geoip_database, get_services_path, save_dockerfile, save_dockerignore,
    save_entrypoint, save_profile, save_secrets, update_ai_config_default_provider,
};
use super::{CreateArgs, TenantTypeArg};
use crate::interactive::Prompter;
use systemprompt_cloud::profile_authoring::{CloudProfileBuilder, LocalProfileBuilder};

#[derive(Debug)]
pub struct CreatedProfile {
    pub name: ProfileName,
}

pub fn create_profile_for_tenant(
    prompter: &dyn Prompter,
    tenant: &StoredTenant,
    api_keys: &ApiKeys,
    profile_name: &ProfileName,
    control_plane_api_url: Option<&str>,
) -> Result<CreatedProfile> {
    let ctx = ProjectContext::discover();
    let name = resolve_unique_profile_name(prompter, &ctx, profile_name)?;
    let profile_dir = ctx.profile_dir(name.as_str());

    std::fs::create_dir_all(ctx.profiles_dir())
        .with_context(|| format!("Failed to create {}", ctx.profiles_dir().display()))?;
    ensure_profile_dirs(&ctx, &profile_dir)?;

    write_profile_secrets(tenant, api_keys, &profile_dir)?;
    update_ai_config_default_provider(api_keys.selected_provider())?;

    let profile_path = ProfilePath::Config.resolve(&profile_dir);
    let built_profile =
        build_tenant_profile(tenant, name.as_str(), control_plane_api_url, &profile_path)?;

    save_profile(&built_profile, &profile_path)?;
    CliService::success(&format!("Created: {}", profile_path.display()));

    write_docker_assets(&ctx, &name)?;
    report_profile_validation(&built_profile);

    Ok(CreatedProfile { name })
}

fn resolve_unique_profile_name(
    prompter: &dyn Prompter,
    ctx: &ProjectContext,
    profile_name: &ProfileName,
) -> Result<ProfileName> {
    let mut name = profile_name.clone();

    loop {
        let profile_dir = ctx.profile_dir(name.as_str());
        if !profile_dir.exists() {
            return Ok(name);
        }

        CliService::warning(&format!(
            "Profile '{}' already exists at {}",
            name,
            profile_dir.display()
        ));

        name = prompt_profile_name(prompter, "Enter a different profile name")?;
    }
}

fn prompt_profile_name(prompter: &dyn Prompter, prompt: &str) -> Result<ProfileName> {
    loop {
        let input = prompter.input(prompt)?;
        match ProfileName::try_new(input) {
            Ok(name) => return Ok(name),
            Err(e) => CliService::warning(&format!("Invalid profile name: {e}")),
        }
    }
}

pub(super) fn ensure_profile_dirs(ctx: &ProjectContext, profile_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(profile_dir)
        .with_context(|| format!("Failed to create directory {}", profile_dir.display()))?;

    std::fs::create_dir_all(ctx.storage_dir()).with_context(|| {
        format!(
            "Failed to create storage directory {}",
            ctx.storage_dir().display()
        )
    })?;

    Ok(())
}

pub fn write_profile_secrets(
    tenant: &StoredTenant,
    api_keys: &ApiKeys,
    profile_dir: &Path,
) -> Result<()> {
    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);
    let local_db_url = tenant
        .get_local_database_url()
        .ok_or_else(|| anyhow::anyhow!("Tenant database URL is required"))?;
    let db_urls = DatabaseUrls {
        external: local_db_url,
        internal: tenant.internal_database_url.as_deref(),
    };
    save_secrets(
        &db_urls,
        api_keys,
        &secrets_path,
        tenant.tenant_type == TenantType::Cloud,
    )?;
    CliService::success(&format!("Created: {}", secrets_path.display()));
    Ok(())
}

fn build_tenant_profile(
    tenant: &StoredTenant,
    name: &str,
    control_plane_api_url: Option<&str>,
    profile_path: &Path,
) -> Result<Profile> {
    Ok(match tenant.tenant_type {
        TenantType::Local => {
            let services_path = get_services_path()?;
            LocalProfileBuilder::new(name, "./secrets.json", &services_path)
                .with_tenant_id(tenant.id.clone())
                .build()
        },
        TenantType::Cloud => {
            let mut builder = CloudProfileBuilder::new(name)
                .with_tenant_id(tenant.id.clone())
                .with_external_db_access(tenant.external_db_access)
                .with_secrets_path("./secrets.json")
                .with_geoip_database(existing_geoip_database(profile_path));
            if let Some(hostname) = &tenant.hostname {
                builder = builder.with_external_url(format!("https://{}", hostname));
            }
            if let Some(api_url) = control_plane_api_url {
                let trimmed = api_url.trim_end_matches('/').to_owned();
                builder = builder.with_trusted_issuer(TrustedIssuer {
                    issuer: trimmed.clone(),
                    jwks_uri: format!("{}/.well-known/jwks.json", trimmed),
                    audience: tenant.id.as_str().to_owned(),
                    typ_allowlist: Vec::new(),
                    allowed_client_ids: Vec::new(),
                    can_issue_id_jag: false,
                });
            }
            builder.build()
        },
    })
}

pub(super) fn write_docker_assets(ctx: &ProjectContext, name: &ProfileName) -> Result<()> {
    let docker_dir = ctx.profile_docker_dir(name.as_str());
    std::fs::create_dir_all(&docker_dir)
        .with_context(|| format!("Failed to create docker directory {}", docker_dir.display()))?;

    let dockerfile_path = ctx.profile_dockerfile(name.as_str());
    save_dockerfile(&dockerfile_path, name, ctx.root())?;
    CliService::success(&format!("Created: {}", dockerfile_path.display()));

    let entrypoint_path = ctx.profile_entrypoint(name.as_str());
    save_entrypoint(&entrypoint_path)?;
    CliService::success(&format!("Created: {}", entrypoint_path.display()));

    let dockerignore_path = ctx.profile_dockerignore(name.as_str());
    save_dockerignore(&dockerignore_path)?;
    CliService::success(&format!("Created: {}", dockerignore_path.display()));

    Ok(())
}

pub(super) fn report_profile_validation(profile: &Profile) {
    match profile.validate() {
        Ok(()) => CliService::success("Profile validated"),
        Err(e) => CliService::warning(&format!("Validation warning: {}", e)),
    }
}

pub fn resolve_tenant_from_args(args: &CreateArgs, store: &TenantStore) -> Result<StoredTenant> {
    let tenant_id = args.tenant.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "Missing required flag: --tenant-id\nIn non-interactive mode, --tenant-id is \
             required.\nList tenants with: systemprompt cloud tenant list"
        )
    })?;

    let tenant = store.find_tenant(tenant_id).ok_or_else(|| {
        anyhow::anyhow!(
            "Tenant '{}' not found.\nList available tenants with: systemprompt cloud tenant list",
            tenant_id
        )
    })?;

    let expected_type: TenantType = match args.tenant_type {
        TenantTypeArg::Local => TenantType::Local,
        TenantTypeArg::Cloud => TenantType::Cloud,
    };

    if tenant.tenant_type != expected_type {
        bail!(
            "Tenant '{}' is type {:?}, but --tenant-type {:?} was specified",
            tenant_id,
            tenant.tenant_type,
            args.tenant_type
        );
    }

    Ok(tenant.clone())
}
