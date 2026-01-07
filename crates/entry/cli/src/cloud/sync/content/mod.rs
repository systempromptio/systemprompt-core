mod display;

use crate::cloud::sync::ContentSyncArgs;
use anyhow::{Context, Result};
use dialoguer::{Confirm, Select};
use std::sync::Arc;
use systemprompt_core_database::{Database, DbPool};
use systemprompt_core_logging::CliService;
use systemprompt_models::{AppPaths, ContentConfigRaw, ContentSourceConfigRaw, SecretsBootstrap};
use systemprompt_sync::{ContentDiffEntry, ContentLocalSync, LocalSyncDirection};

fn get_content_config_path() -> Result<std::path::PathBuf> {
    let paths = AppPaths::get().map_err(|e| anyhow::anyhow!("{}", e))?;
    let path = paths.system().content_config().to_path_buf();

    if !path.exists() {
        anyhow::bail!(
            "Profile Error: Content config path does not exist\n\n  Path: {}\n  Field: \
             paths.content_config\n\n  To fix: Ensure the path exists or update your profile",
            path.display()
        );
    }
    Ok(path)
}

fn load_content_config() -> Result<ContentConfigRaw> {
    let config_path = get_content_config_path()?;
    let content = std::fs::read_to_string(&config_path)
        .context(format!("Failed to read config: {}", config_path.display()))?;
    let config: ContentConfigRaw =
        serde_yaml::from_str(&content).context("Failed to parse content config")?;
    Ok(config)
}

async fn create_db_pool(database_url: Option<&str>) -> Result<DbPool> {
    let url = match database_url {
        Some(url) => url.to_string(),
        None => SecretsBootstrap::database_url()?.to_string(),
    };

    let database = Database::from_config("postgres", &url)
        .await
        .context("Failed to connect to database")?;

    Ok(Arc::new(database))
}

pub async fn execute(args: ContentSyncArgs) -> Result<()> {
    CliService::section("Content Sync");

    let spinner = CliService::spinner("Connecting to database...");
    let db = create_db_pool(args.database_url.as_deref()).await?;
    spinner.finish_and_clear();

    let config = load_content_config()?;

    let sources: Vec<(String, ContentSourceConfigRaw)> = config
        .content_sources
        .into_iter()
        .filter(|(_, source)| source.enabled)
        .filter(|(name, _)| {
            args.source
                .as_ref()
                .map_or(true, |filter| name.as_str() == filter.as_str())
        })
        .filter(|(_, source)| !source.allowed_content_types.contains(&"skill".to_string()))
        .collect();

    if sources.is_empty() {
        if let Some(ref filter) = args.source {
            CliService::warning(&format!("No content source found matching: {}", filter));
        } else {
            CliService::warning("No enabled content sources found");
        }
        return Ok(());
    }

    let sync = ContentLocalSync::new(Arc::clone(&db));
    let mut all_diffs: Vec<ContentDiffEntry> = Vec::new();

    let spinner = CliService::spinner("Calculating diff...");
    for (name, source) in sources {
        let base_path = std::env::current_dir()?;
        let source_path = base_path.join(&source.path);

        let diff = sync
            .calculate_diff(
                source.source_id.as_str(),
                &source_path,
                &source.allowed_content_types,
            )
            .await
            .context(format!("Failed to calculate diff for source: {}", name))?;

        all_diffs.push(ContentDiffEntry {
            name,
            source_id: source.source_id.to_string(),
            category_id: source.category_id.to_string(),
            path: source_path,
            allowed_content_types: source.allowed_content_types.clone(),
            diff,
        });
    }
    spinner.finish_and_clear();

    display::display_diff_summary(&all_diffs);

    let has_changes = all_diffs.iter().any(|e| e.diff.has_changes());

    if !has_changes {
        CliService::success("Content is in sync - no changes needed");
        return Ok(());
    }

    let direction = match args.direction {
        Some(crate::cloud::sync::CliLocalSyncDirection::ToDisk) => LocalSyncDirection::ToDisk,
        Some(crate::cloud::sync::CliLocalSyncDirection::ToDb) => LocalSyncDirection::ToDatabase,
        None => {
            if let Some(dir) = prompt_sync_direction()? {
                dir
            } else {
                CliService::info("Sync cancelled");
                return Ok(());
            }
        },
    };

    if args.dry_run {
        CliService::info("Dry run - no changes made");
        return Ok(());
    }

    if args.direction.is_none() {
        let confirmed = Confirm::new()
            .with_prompt("Proceed with sync?")
            .default(false)
            .interact()?;

        if !confirmed {
            CliService::info("Sync cancelled");
            return Ok(());
        }
    }

    let spinner = CliService::spinner("Syncing content...");
    let result = match direction {
        LocalSyncDirection::ToDisk => sync.sync_to_disk(&all_diffs, args.delete_orphans).await?,
        LocalSyncDirection::ToDatabase => sync.sync_to_db(&all_diffs, args.delete_orphans).await?,
    };
    spinner.finish_and_clear();

    display::display_sync_result(&result);

    Ok(())
}

fn prompt_sync_direction() -> Result<Option<LocalSyncDirection>> {
    let options = vec![
        "Sync to disk (DB -> Disk)",
        "Sync to database (Disk -> DB)",
        "Cancel",
    ];

    let selection = Select::new()
        .with_prompt("Choose sync direction")
        .items(&options)
        .default(0)
        .interact()?;

    match selection {
        0 => Ok(Some(LocalSyncDirection::ToDisk)),
        1 => Ok(Some(LocalSyncDirection::ToDatabase)),
        _ => Ok(None),
    }
}
