//! Five-second node-local evaluator supervisor tick.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::EvalWorkerId;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderResult};

use crate::services::evaluator::supervisor::{EvaluatorSupervisor, EvaluatorSupervisorConfig};
use crate::SchedulerError;

#[derive(Debug, Clone, Copy)]
pub struct EvaluationSupervisorJob;

#[async_trait]
impl Job for EvaluationSupervisorJob {
    fn name(&self) -> &'static str { "evaluation_supervisor" }
    fn description(&self) -> &'static str { "Claims and supervises frozen evaluator assignments with fenced leases and isolated Docker networks" }
    fn schedule(&self) -> &'static str { "*/5 * * * * *" }
    fn scope(&self) -> JobScope { JobScope::Node }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let required = ["SYSTEMPROMPT_EVALUATOR_WORKER_ID", "SYSTEMPROMPT_EVALUATOR_CLIENT_IMAGE", "SYSTEMPROMPT_EVALUATOR_RELAY_IMAGE", "SYSTEMPROMPT_EVALUATOR_CONTROL_NETWORK"];
        if required.iter().any(|name| std::env::var(name).is_err()) {
            return Ok(JobResult::success().with_message("Evaluator supervisor is idle until its worker and pinned images are configured"));
        }
        let pool = Arc::clone(ctx.db_pool::<DbPool>().ok_or_else(|| SchedulerError::missing_context("DbPool"))?);
        let app = Arc::clone(ctx.app_context::<Arc<AppContext>>().ok_or_else(|| SchedulerError::missing_context("AppContext"))?);
        let supervisor = EvaluatorSupervisor::new(&pool, EvaluatorSupervisorConfig {
            docker: PathBuf::from(std::env::var("SYSTEMPROMPT_EVALUATOR_DOCKER").unwrap_or_else(|_| "/usr/bin/docker".to_owned())),
            workspace_root: PathBuf::from(std::env::var("SYSTEMPROMPT_EVALUATOR_WORKSPACES").unwrap_or_else(|_| "/var/lib/systemprompt/evaluator".to_owned())),
            environment: app.config().api_external_url.clone(),
            client_image: std::env::var("SYSTEMPROMPT_EVALUATOR_CLIENT_IMAGE").map_err(|error| SchedulerError::config_error(error.to_string()))?,
            relay_image: std::env::var("SYSTEMPROMPT_EVALUATOR_RELAY_IMAGE").map_err(|error| SchedulerError::config_error(error.to_string()))?,
            relay_control_network: std::env::var("SYSTEMPROMPT_EVALUATOR_CONTROL_NETWORK").map_err(|error| SchedulerError::config_error(error.to_string()))?,
            relay_upstream: app.config().api_external_url.clone(),
        })?;
        std::fs::create_dir_all(&supervisor.config.workspace_root)?;
        let processed = supervisor.run_once(&ctx.actor().user_id, &EvalWorkerId::new(std::env::var("SYSTEMPROMPT_EVALUATOR_WORKER_ID").map_err(|error| SchedulerError::config_error(error.to_string()))?)).await?;
        Ok(JobResult::success().with_stats(u64::from(processed), 0))
    }
}

systemprompt_provider_contracts::submit_job!(&EvaluationSupervisorJob);
