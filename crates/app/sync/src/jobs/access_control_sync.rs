//! Bootstrap job that ingests the access-control YAML baseline into the
//! database.
//!
//! Mirrors [`super::ContentSyncJob`] but with a fixed direction
//! (YAML → DB). Disabled by default; operators wire it in via
//! `scheduler_config.bootstrap_jobs` so it runs once at startup.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::{Database, DbPool};
use systemprompt_models::AppPaths;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderError, ProviderResult};

use crate::local::AccessControlLocalSync;

const DEFAULT_YAML_RELATIVE: &str = "access-control/config.yaml";

#[derive(Debug, Clone, Copy)]
pub struct AccessControlSyncJob;

#[async_trait]
impl Job for AccessControlSyncJob {
    fn name(&self) -> &'static str {
        "access_control_sync"
    }

    fn description(&self) -> &'static str {
        "Project services/access-control YAML into access_control_rules"
    }

    fn schedule(&self) -> &'static str {
        ""
    }

    fn tags(&self) -> Vec<&'static str> {
        vec!["access-control", "sync", "bootstrap"]
    }

    fn enabled(&self) -> bool {
        false
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start = std::time::Instant::now();

        let db_pool: &DbPool = ctx.db_pool::<DbPool>().ok_or_else(|| {
            ProviderError::Configuration("DbPool not available in job context".into())
        })?;

        let paths = ctx
            .app_paths::<Arc<AppPaths>>()
            .ok_or_else(|| {
                ProviderError::Configuration("AppPaths not available in job context".into())
            })?
            .as_ref();

        let yaml_path = resolve_yaml_path(ctx, paths.system().services());
        let override_existing = bool_param(ctx, "override_existing", true);
        let delete_orphans = bool_param(ctx, "delete_orphans", true);

        tracing::info!(
            yaml_path = %yaml_path.display(),
            override_existing,
            delete_orphans,
            "access_control_sync job started",
        );

        let sync = AccessControlLocalSync::new(Arc::<Database>::clone(db_pool), yaml_path);
        let result = sync
            .sync_to_db(override_existing, delete_orphans)
            .await
            .map_err(|e| ProviderError::RenderFailed(e.to_string()))?;

        let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::info!(
            items_synced = result.items_synced,
            items_skipped = result.items_skipped,
            items_deleted = result.items_deleted,
            duration_ms,
            "access_control_sync job completed",
        );

        Ok(JobResult::success()
            .with_stats(result.items_synced as u64, result.errors.len() as u64)
            .with_duration(duration_ms))
    }
}

fn resolve_yaml_path(ctx: &JobContext, services_path: &std::path::Path) -> PathBuf {
    ctx.parameters().get("yaml_path").map_or_else(
        || services_path.join(DEFAULT_YAML_RELATIVE),
        |raw| {
            let p = std::path::Path::new(raw);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                services_path.join(p)
            }
        },
    )
}

fn bool_param(ctx: &JobContext, key: &str, default: bool) -> bool {
    ctx.parameters().get(key).map_or(default, |v| {
        matches!(v.as_str(), "true" | "1" | "yes" | "TRUE" | "True")
    })
}

systemprompt_provider_contracts::submit_job!(&AccessControlSyncJob);
