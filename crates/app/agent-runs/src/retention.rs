//! Finished runs keep their directory (logs, `outputs/`) on disk only for
//! `profile.retention.agent_run_dirs_days`; the row outlives it.
//!
//! The job reads the retention window as [`AgentRunRetention`], which the
//! agent-runs member narrows from the sealed runtime `Config` in its provide
//! fn; no job context carries the `Config`.
//!
//! Only a directory under the runner's runs root is removed, so a row whose
//! `run_dir` points elsewhere is cleared without touching the filesystem.
//! Without `enforce` the job reports what it would remove.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use systemprompt_config::paths::AppPaths;
use systemprompt_manifest::Config;
use systemprompt_provider_contracts::{ProviderError, ProviderResult};
use systemprompt_traits::{Job, JobContext, JobResult};

use crate::repository::AgentRunRepository;
use crate::runner::A2aRunnerConfig;

const BATCH: i64 = 200;

/// How long a finished run keeps its directory, narrowed from the runtime
/// configuration's retention section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentRunRetention {
    days: u32,
}

impl AgentRunRetention {
    #[must_use]
    pub fn of(config: &Config) -> Self {
        Self {
            days: config.retention.agent_run_dirs_days,
        }
    }

    #[must_use]
    pub const fn days(self) -> u32 {
        self.days
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AgentRunDirsCleanupJob;

#[async_trait]
impl Job for AgentRunDirsCleanupJob {
    fn name(&self) -> &'static str {
        "agent_run_dirs_cleanup"
    }

    fn description(&self) -> &'static str {
        "Removes finished agent runs' directories past profile.retention.agent_run_dirs_days; removal requires enforce"
    }

    fn schedule(&self) -> &'static str {
        "0 30 3 * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let start = std::time::Instant::now();
        let paths = Arc::clone(ctx.get::<Arc<AppPaths>>()?);
        let days = ctx.get::<AgentRunRetention>()?.days();
        let cutoff = Utc::now() - chrono::Duration::days(i64::from(days));
        let runs_root = A2aRunnerConfig::runs_root(paths.storage().data());
        let repo = ctx.get::<AgentRunRepository>()?;
        let expired = repo
            .list_expired_run_dirs(cutoff, BATCH)
            .await
            .map_err(|e| ProviderError::Internal(Box::new(e)))?;
        if !ctx.enforce() {
            tracing::info!(
                expired = expired.len(),
                retention_days = days,
                "enforce disabled: expired run directories were not removed"
            );
            return Ok(JobResult::success().with_stats(0, 0));
        }
        let mut removed = 0u64;
        let mut failed = 0u64;
        for run in expired {
            let dir = Path::new(&run.run_dir);
            if dir.starts_with(&runs_root) {
                match tokio::fs::remove_dir_all(dir).await {
                    Ok(()) => removed += 1,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
                    Err(error) => {
                        failed += 1;
                        tracing::warn!(run_id = %run.id, %error, "run directory not removed");
                        continue;
                    },
                }
            }
            repo.clear_run_dir(&run.id)
                .await
                .map_err(|e| ProviderError::Internal(Box::new(e)))?;
        }
        tracing::info!(
            removed,
            failed,
            retention_days = days,
            "agent run directories pruned"
        );
        Ok(JobResult::success()
            .with_stats(removed, failed)
            .with_duration(start.elapsed().as_millis() as u64))
    }
}
