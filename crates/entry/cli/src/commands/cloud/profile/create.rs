use anyhow::{bail, Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;
use systemprompt_cloud::{
    get_cloud_paths, CloudApiClient, CloudPath, ProfilePath, ProjectContext, StoredTenant,
    TenantStore, TenantType,
};
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use systemprompt_identifiers::TenantId;

use crate::commands::cloud::tenant::get_credentials;

use super::api_keys::{collect_api_keys, ApiKeys};
use super::builders::{CloudProfileBuilder, LocalProfileBuilder};
use super::create_setup::{get_cloud_user, handle_local_tenant_setup};
use super::create_tenant::{get_tenants_by_type, select_tenant, select_tenant_type};
use super::templates::{
    get_services_path, save_dockerfile, save_dockerignore, save_entrypoint, save_profile,
    save_secrets, DatabaseUrls,
};
use super::{CreateArgs, TenantTypeArg};
use crate::cli_settings::CliConfig;

pub async fn execute(args: &CreateArgs, config: &CliConfig) -> Result<()> {
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

    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        CliService::warning(&format!("Failed to load tenant store: {}", e));
        TenantStore::default()
    });

    let (tenant, api_keys) = if config.is_interactive() && args.tenant_id.is_none() {
        let tenant_type = select_tenant_type(&store)?;
        let eligible_tenants = get_tenants_by_type(&store, tenant_type);
        let tenant = select_tenant(&eligible_tenants)?;

        if !tenant.has_database_url() {
            bail!(
                "Tenant '{}' does not have a database URL configured.\nFor local tenants, \
                 recreate with 'systemprompt cloud tenant create'.",
                tenant.name
            );
        }

        CliService::section("API Keys");
        let api_keys = collect_api_keys()?;
        (tenant, api_keys)
    } else {
        let tenant = resolve_tenant_from_args(args, &store)?;

        if !tenant.has_database_url() {
            bail!(
                "Tenant '{}' does not have a database URL configured.\nFor local tenants, \
                 recreate with 'systemprompt cloud tenant create'.",
                tenant.name
            );
        }

        let api_keys = ApiKeys::from_options(
            args.gemini_key.clone(),
            args.anthropic_key.clone(),
            args.openai_key.clone(),
        )?;
        (tenant, api_keys)
    };

    let tenant = ensure_unmasked_credentials(tenant, &tenants_path).await?;

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
    save_secrets(
        &db_urls,
        &api_keys,
        tenant.sync_token.as_deref(),
        &secrets_path,
    )?;
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
    save_secrets(
        &db_urls,
        api_keys,
        tenant.sync_token.as_deref(),
        &secrets_path,
    )?;
    CliService::success(&format!("Created: {}", secrets_path.display()));

    let profile_path = ProfilePath::Config.resolve(&profile_dir);

    let built_profile = match tenant.tenant_type {
        TenantType::Local => {
            let services_path = get_services_path()?;
            LocalProfileBuilder::new(&name, "./secrets.json", &services_path)
                .with_tenant_id(TenantId::new(&tenant.id))
                .build()
        },
        TenantType::Cloud => {
            let mut builder = CloudProfileBuilder::new(&name)
                .with_tenant_id(TenantId::new(&tenant.id))
                .with_external_db_access(tenant.external_db_access)
                .with_secrets_path("./secrets.json");
            if let Some(hostname) = &tenant.hostname {
                builder = builder.with_external_url(format!("https://{}", hostname));
            }
            builder.build()
        },
    };

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

fn resolve_tenant_from_args(args: &CreateArgs, store: &TenantStore) -> Result<StoredTenant> {
    let tenant_id = args.tenant_id.as_ref().ok_or_else(|| {
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

struct RefreshedCredentials {
    external_database_url: String,
    internal_database_url: String,
    sync_token: Option<String>,
}

async fn refresh_tenant_credentials(
    client: &CloudApiClient,
    tenant_id: &str,
) -> Result<RefreshedCredentials> {
    let status = client.get_tenant_status(tenant_id).await?;
    let secrets_url = status
        .secrets_url
        .ok_or_else(|| anyhow::anyhow!("No secrets URL available for tenant"))?;
    let secrets = client.fetch_secrets(&secrets_url).await?;
    Ok(RefreshedCredentials {
        external_database_url: secrets.database_url,
        internal_database_url: secrets.internal_database_url,
        sync_token: secrets.sync_token,
    })
}

async fn ensure_unmasked_credentials(
    tenant: StoredTenant,
    tenants_path: &std::path::Path,
) -> Result<StoredTenant> {
    if tenant.tenant_type != TenantType::Cloud {
        return Ok(tenant);
    }

    let external_url = tenant.database_url.as_deref();
    let internal_url = tenant.internal_database_url.as_deref();

    let needs_external = tenant.external_db_access && external_url.is_none();
    let needs_refresh = needs_external
        || external_url.is_some_and(Profile::is_masked_database_url)
        || internal_url.is_none_or(Profile::is_masked_database_url);

    if !needs_refresh {
        return Ok(tenant);
    }

    CliService::info("Fetching database credentials...");
    let creds = get_credentials()?;
    let client = CloudApiClient::new(&creds.api_url, &creds.api_token);

    match refresh_tenant_credentials(&client, &tenant.id).await {
        Ok(creds) => {
            let mut updated_tenant = tenant.clone();
            updated_tenant.internal_database_url = Some(creds.internal_database_url);
            if updated_tenant.external_db_access {
                updated_tenant.database_url = Some(creds.external_database_url);
            }
            if let Some(token) = creds.sync_token {
                updated_tenant.sync_token = Some(token);
            }

            let mut store = TenantStore::load_from_path(tenants_path)
                .unwrap_or_else(|_| TenantStore::default());
            if let Some(t) = store.tenants.iter_mut().find(|t| t.id == tenant.id) {
                *t = updated_tenant.clone();
                store.save_to_path(tenants_path)?;
            }

            CliService::success("Database credentials retrieved");
            Ok(updated_tenant)
        },
        Err(e) => {
            CliService::warning(&format!("Could not fetch credentials: {}", e));
            CliService::warning(
                "Run 'systemprompt cloud tenant rotate-credentials' to fetch real credentials.",
            );
            Ok(tenant)
        },
    }
}
