//! Job converging every `file`-authority plane to its declaration (file
//! wins, see [`crate::services::ConfigApplyService::apply_file_managed`]),
//! then reporting every declared plane's drift (see
//! [`crate::services::ConfigReconcileService`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use systemprompt_provider_contracts::ChangeActor;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};

use crate::repository::ConfigReconcileRepository;
use crate::services::{
    ConfigApplyService, ConfigReconcileService, RECONCILE_LOCK_WAIT, ReconcileTrigger,
};

#[derive(Debug, Clone, Copy)]
pub struct ConfigReconcileJob;

#[async_trait]
impl Job for ConfigReconcileJob {
    fn name(&self) -> &'static str {
        "config_reconcile"
    }

    fn description(&self) -> &'static str {
        "Applies file-authority config planes (file wins) and reports drift on the rest"
    }

    fn schedule(&self) -> &'static str {
        "0 * * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let started = Instant::now();
        let app = ctx.get::<Arc<AppContext>>()?;
        let service = ConfigReconcileService::from_app_context(
            app,
            ConfigReconcileRepository::new(app.db_pool()),
        )?;
        let applier = ConfigApplyService::new(service);
        let file_planes = applier
            .apply_file_managed(
                ChangeActor::Reconciler,
                "file-authority planes re-applied",
                RECONCILE_LOCK_WAIT,
            )
            .await?;
        let report = applier
            .reconcile()
            .reconcile_all(ReconcileTrigger::Scheduled)
            .await;
        let failures = report.failures();
        let duration_ms = started.elapsed().as_millis() as u64;
        if failures.is_empty() {
            return Ok(JobResult::success()
                .with_stats((report.drift_rows() + file_planes.applied()) as u64, 0)
                .with_duration(duration_ms));
        }
        let planes: Vec<&str> = failures.iter().map(|p| p.plane.as_str()).collect();
        Ok(JobResult::failure(format!(
            "config planes failed to reconcile: {}",
            planes.join(", ")
        ))
        .with_stats(report.drift_rows() as u64, failures.len() as u64)
        .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&ConfigReconcileJob);
