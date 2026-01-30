use crate::local::{ContentDiffEntry, ContentLocalSync};
use crate::models::LocalSyncDirection;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::{AppPaths, ContentConfigRaw};
use systemprompt_traits::{Job, JobContext, JobResult};

#[derive(Debug, Clone, Copy)]
pub struct ContentSyncJob;

#[async_trait]
impl Job for ContentSyncJob {
    fn name(&self) -> &'static str {
        "content_sync"
    }

    fn description(&self) -> &'static str {
        "Synchronize content between database and filesystem"
    }

    fn schedule(&self) -> &'static str {
        ""
    }

    fn tags(&self) -> Vec<&'static str> {
        vec!["content", "sync"]
    }

    fn enabled(&self) -> bool {
        false
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let start_time = std::time::Instant::now();

        let db_pool = Arc::clone(
            ctx.db_pool::<DbPool>()
                .ok_or_else(|| anyhow::anyhow!("DbPool not available in job context"))?,
        );

        tracing::info!("Content sync job started");

        let direction = get_direction_from_params(ctx)?;
        let delete_orphans = get_bool_param(ctx, "delete_orphans");
        let override_existing = get_bool_param(ctx, "override_existing");

        let config = load_content_config()?;
        let paths = AppPaths::get().map_err(|e| anyhow::anyhow!("{}", e))?;
        let services_path = paths.system().services();

        let sources: Vec<_> = config
            .content_sources
            .into_iter()
            .filter(|(_, source)| source.enabled)
            .filter(|(_, source)| !source.allowed_content_types.contains(&"skill".to_string()))
            .collect();

        if sources.is_empty() {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            tracing::warn!("No enabled content sources found");
            return Ok(JobResult::success()
                .with_message("No enabled content sources")
                .with_duration(duration_ms));
        }

        let sync = ContentLocalSync::new(db_pool);
        let mut all_diffs: Vec<ContentDiffEntry> = Vec::new();

        for (name, source) in sources {
            let source_path = resolve_source_path(&source.path, services_path);

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

        let has_changes = all_diffs.iter().any(|e| e.diff.has_changes());

        if !has_changes {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            tracing::info!("Content is in sync - no changes needed");
            return Ok(JobResult::success()
                .with_message("Content is in sync")
                .with_stats(0, 0)
                .with_duration(duration_ms));
        }

        let result = match direction {
            LocalSyncDirection::ToDisk => sync.sync_to_disk(&all_diffs, delete_orphans).await?,
            LocalSyncDirection::ToDatabase => {
                sync.sync_to_db(&all_diffs, delete_orphans, override_existing)
                    .await?
            },
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        tracing::info!(
            direction = %result.direction,
            items_synced = result.items_synced,
            items_deleted = result.items_deleted,
            items_skipped = result.items_skipped,
            errors = result.errors.len(),
            duration_ms,
            "Content sync job completed"
        );

        Ok(JobResult::success()
            .with_stats(result.items_synced as u64, result.errors.len() as u64)
            .with_duration(duration_ms))
    }
}

fn get_direction_from_params(ctx: &JobContext) -> Result<LocalSyncDirection> {
    let params = ctx.parameters();
    let direction_str = params.get("direction").map_or("to_db", String::as_str);

    match direction_str {
        "to_disk" | "to-disk" | "disk" => Ok(LocalSyncDirection::ToDisk),
        "to_db" | "to-db" | "db" | "to_database" => Ok(LocalSyncDirection::ToDatabase),
        other => anyhow::bail!("Invalid direction '{}'. Use 'to_disk' or 'to_db'", other),
    }
}

fn get_bool_param(ctx: &JobContext, key: &str) -> bool {
    ctx.parameters()
        .get(key)
        .is_some_and(|v| v == "true" || v == "1" || v == "yes")
}

fn load_content_config() -> Result<ContentConfigRaw> {
    let paths = AppPaths::get().map_err(|e| anyhow::anyhow!("{}", e))?;
    let config_path = paths.system().content_config();

    if !config_path.exists() {
        anyhow::bail!("Content config not found at: {}", config_path.display());
    }

    let content = std::fs::read_to_string(config_path).context("Failed to read content config")?;
    let config: ContentConfigRaw =
        serde_yaml::from_str(&content).context("Failed to parse content config")?;
    Ok(config)
}

fn resolve_source_path(path: &str, services_path: &Path) -> std::path::PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        services_path.join(path)
    }
}

systemprompt_provider_contracts::submit_job!(&ContentSyncJob);
