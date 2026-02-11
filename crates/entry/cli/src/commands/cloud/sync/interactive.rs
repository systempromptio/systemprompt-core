use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_loader::ProfileLoader;
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use crate::cli_settings::CliConfig;

use super::{skills, SkillsSyncArgs};

#[derive(Debug, Clone, Copy)]
pub enum SyncType {
    Push,
    Pull,
    LocalSkills,
}

pub struct ProfileSelection {
    pub name: String,
    pub path: PathBuf,
    pub profile: Profile,
}

pub async fn execute(config: &CliConfig) -> Result<()> {
    CliService::section("Sync Menu");

    let sync_type = select_sync_type()?;

    let source = select_profile("Select source profile")?;

    match sync_type {
        SyncType::Push | SyncType::Pull => execute_cloud_sync(sync_type, &source).await,
        SyncType::LocalSkills => execute_local_skills_sync(&source, config).await,
    }
}

fn select_sync_type() -> Result<SyncType> {
    let options = vec![
        "Push to cloud (Local → Cloud)",
        "Pull from cloud (Cloud → Local)",
        "Sync skills (Disk ↔ Database)",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select sync operation")
        .items(&options)
        .default(0)
        .interact()
        .context("Failed to select sync type")?;

    match selection {
        0 => Ok(SyncType::Push),
        1 => Ok(SyncType::Pull),
        2 => Ok(SyncType::LocalSkills),
        _ => bail!("Invalid selection"),
    }
}

fn select_profile(prompt: &str) -> Result<ProfileSelection> {
    let profiles = discover_profiles()?;

    if profiles.is_empty() {
        bail!(
            "No profiles found.\nCreate a profile with: systemprompt cloud profile create <name>"
        );
    }

    let options: Vec<String> = profiles
        .iter()
        .map(|p| {
            let cloud_status = if p.profile.cloud.is_some() {
                "cloud"
            } else {
                "local"
            };
            format!("{} ({})", p.name, cloud_status)
        })
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .items(&options)
        .default(0)
        .interact()
        .context("Failed to select profile")?;

    profiles
        .into_iter()
        .nth(selection)
        .ok_or_else(|| anyhow::anyhow!("Invalid selection index"))
}

fn discover_profiles() -> Result<Vec<ProfileSelection>> {
    let ctx = ProjectContext::discover();
    let profiles_dir = ctx.profiles_dir();

    if !profiles_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = std::fs::read_dir(&profiles_dir).with_context(|| {
        format!(
            "Failed to read profiles directory: {}",
            profiles_dir.display()
        )
    })?;

    let profiles: Vec<ProfileSelection> = entries
        .filter_map(std::result::Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|entry| {
            let profile_yaml = ProfilePath::Config.resolve(&entry.path());
            if !profile_yaml.exists() {
                return None;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let profile = ProfileLoader::load_from_path(&profile_yaml).ok()?;

            Some(ProfileSelection {
                name,
                path: profile_yaml,
                profile,
            })
        })
        .collect();

    Ok(profiles)
}

async fn execute_cloud_sync(sync_type: SyncType, source: &ProfileSelection) -> Result<()> {
    use systemprompt_cloud::{get_cloud_paths, CloudPath, CredentialsBootstrap, TenantStore};
    use systemprompt_sync::{SyncConfig, SyncDirection, SyncOperationResult, SyncService};

    let creds = CredentialsBootstrap::require()
        .context("Cloud sync requires credentials. Run 'systemprompt cloud auth login'")?;

    let cloud = source
        .profile
        .cloud
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Profile has no cloud configuration"))?;

    let tenant_id = cloud
        .tenant_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No tenant configured in profile"))?;

    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let store = TenantStore::load_from_path(&tenants_path).unwrap_or_else(|e| {
        CliService::warning(&format!("Failed to load tenant store: {}", e));
        TenantStore::default()
    });
    let tenant = store.find_tenant(tenant_id);

    let (hostname, sync_token) =
        tenant.map_or((None, None), |t| (t.hostname.clone(), t.sync_token.clone()));

    let direction = match sync_type {
        SyncType::Push => SyncDirection::Push,
        SyncType::Pull => SyncDirection::Pull,
        SyncType::LocalSkills => bail!("Invalid sync type for cloud sync"),
    };

    let config = SyncConfig {
        direction,
        dry_run: false,
        verbose: false,
        tenant_id: tenant_id.clone(),
        api_url: creds.api_url.clone(),
        api_token: creds.api_token.clone(),
        services_path: source.profile.paths.services.clone(),
        hostname,
        sync_token,
        local_database_url: None,
    };

    let dir_label = match direction {
        SyncDirection::Push => "Local → Cloud",
        SyncDirection::Pull => "Cloud → Local",
    };
    CliService::key_value("Direction", dir_label);

    let service = SyncService::new(config)?;
    let mut results: Vec<SyncOperationResult> = Vec::new();

    let spinner = CliService::spinner("Syncing files...");
    let files_result = service.sync_files().await?;
    spinner.finish_and_clear();
    results.push(files_result);

    for result in &results {
        if result.success {
            CliService::success(&format!(
                "{} - Synced {} items",
                result.operation, result.items_synced
            ));
        } else {
            CliService::error(&format!(
                "{} - Failed with {} errors",
                result.operation,
                result.errors.len()
            ));
        }
    }

    Ok(())
}

async fn execute_local_skills_sync(source: &ProfileSelection, config: &CliConfig) -> Result<()> {
    std::env::set_var("SYSTEMPROMPT_PROFILE", &source.path);

    let args = SkillsSyncArgs {
        direction: None,
        database_url: None,
        skill: None,
        dry_run: false,
        delete_orphans: false,
        yes: false,
    };

    skills::execute(args, config).await
}
