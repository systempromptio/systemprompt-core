use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_core_logging::CliService;
use systemprompt_loader::ProfileLoader;
use systemprompt_models::Profile;

use super::{content, skills, ContentSyncArgs, SkillsSyncArgs};

#[derive(Debug, Clone, Copy)]
pub enum SyncType {
    Push,
    Pull,
    LocalContent,
    LocalSkills,
}

pub struct ProfileSelection {
    pub name: String,
    pub path: PathBuf,
    pub profile: Profile,
}

pub async fn execute() -> Result<()> {
    CliService::section("Sync Menu");

    let sync_type = select_sync_type()?;

    let source = select_profile("Select source profile")?;

    match sync_type {
        SyncType::Push | SyncType::Pull => execute_cloud_sync(sync_type, &source).await,
        SyncType::LocalContent => execute_local_content_sync(&source).await,
        SyncType::LocalSkills => execute_local_skills_sync(&source).await,
    }
}

fn select_sync_type() -> Result<SyncType> {
    let options = vec![
        "Push to cloud (Local → Cloud)",
        "Pull from cloud (Cloud → Local)",
        "Sync content (Disk ↔ Database)",
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
        2 => Ok(SyncType::LocalContent),
        3 => Ok(SyncType::LocalSkills),
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
            let cloud_status = p.profile.cloud.as_ref().map_or("local", |c| {
                if c.cli_enabled {
                    "cloud"
                } else {
                    "local"
                }
            });
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
    use systemprompt_cloud::CredentialsBootstrap;
    use systemprompt_sync::{SyncConfig, SyncDirection, SyncService};

    let creds = CredentialsBootstrap::require()
        .context("Cloud sync requires credentials. Run 'systemprompt cloud auth login'")?;

    let cloud = source
        .profile
        .cloud
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Profile has no cloud configuration"))?;

    if !cloud.cli_enabled {
        bail!("Cloud features are disabled in this profile");
    }

    let tenant_id = cloud
        .tenant_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No tenant configured in profile"))?;

    let direction = match sync_type {
        SyncType::Push => SyncDirection::Push,
        SyncType::Pull => SyncDirection::Pull,
        _ => bail!("Invalid sync type for cloud sync"),
    };

    let secrets_path = source
        .path
        .parent()
        .map(|p| ProfilePath::Secrets.resolve(p));

    let database_url = secrets_path
        .and_then(|p| std::fs::read_to_string(&p).ok())
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
        .and_then(|v| {
            v.get("database_url")
                .and_then(|u| u.as_str())
                .map(String::from)
        });

    let config = SyncConfig {
        direction,
        dry_run: false,
        verbose: false,
        tenant_id: tenant_id.clone(),
        api_url: creds.api_url.clone(),
        api_token: creds.api_token.clone(),
        services_path: source.profile.paths.services.clone(),
        database_url,
    };

    let dir_label = match direction {
        SyncDirection::Push => "Local → Cloud",
        SyncDirection::Pull => "Cloud → Local",
    };
    CliService::key_value("Direction", dir_label);

    let service = SyncService::new(config);

    let spinner_msg = match direction {
        SyncDirection::Push => "Pushing to cloud...",
        SyncDirection::Pull => "Pulling from cloud...",
    };
    let spinner = CliService::spinner(spinner_msg);

    let results = service.sync_all().await?;

    spinner.finish_and_clear();

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

async fn execute_local_content_sync(source: &ProfileSelection) -> Result<()> {
    std::env::set_var("SYSTEMPROMPT_PROFILE", &source.path);

    let args = ContentSyncArgs {
        direction: None,
        database_url: None,
        source: None,
        dry_run: false,
        delete_orphans: false,
    };

    content::execute(args).await
}

async fn execute_local_skills_sync(source: &ProfileSelection) -> Result<()> {
    std::env::set_var("SYSTEMPROMPT_PROFILE", &source.path);

    let args = SkillsSyncArgs {
        direction: None,
        database_url: None,
        skill: None,
        dry_run: false,
        delete_orphans: false,
    };

    skills::execute(args).await
}
