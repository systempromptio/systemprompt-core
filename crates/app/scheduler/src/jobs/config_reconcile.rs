//! Job reconciling every declared configuration plane to its authority (see
//! [`crate::services::ConfigReconcileService`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};

use crate::repository::ConfigReconcileRepository;
use crate::services::{ConfigReconcileService, ReconcileTrigger};

#[derive(Debug, Clone, Copy)]
pub struct ConfigReconcileJob;

#[async_trait]
impl Job for ConfigReconcileJob {
    fn name(&self) -> &'static str {
        "config_reconcile"
    }

    fn description(&self) -> &'static str {
        "Overwrites database drift on file-authority config planes and reports drift on the rest"
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
        let report = service.reconcile_all(ReconcileTrigger::Scheduled).await;
        let failures = report.failures();
        let duration_ms = started.elapsed().as_millis() as u64;
        if failures.is_empty() {
            return Ok(JobResult::success()
                .with_stats(report.changes() as u64, 0)
                .with_duration(duration_ms));
        }
        let planes: Vec<&str> = failures.iter().map(|p| p.plane.as_str()).collect();
        Ok(JobResult::failure(format!(
            "config planes failed to reconcile: {}",
            planes.join(", ")
        ))
        .with_stats(report.changes() as u64, failures.len() as u64)
        .with_duration(duration_ms))
    }
}

systemprompt_provider_contracts::submit_job!(&ConfigReconcileJob);
