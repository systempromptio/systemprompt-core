//! Five-second node-local evaluator supervisor tick.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::sync::Arc;
use systemprompt_config::ProfileBootstrap;
use systemprompt_evaluation::campaigns::repository::CampaignRepository;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Job, JobContext, JobResult, JobScope, ProviderResult};

use crate::SchedulerError;
use crate::services::evaluator::supervisor::{EvaluatorSupervisor, EvaluatorSupervisorConfig};

#[derive(Debug, Clone, Copy)]
pub struct EvaluationSupervisorJob;

#[async_trait]
impl Job for EvaluationSupervisorJob {
    fn name(&self) -> &'static str {
        "evaluation_supervisor"
    }
    fn description(&self) -> &'static str {
        "Claims and supervises frozen evaluator assignments with fenced leases and isolated Docker networks"
    }
    fn schedule(&self) -> &'static str {
        "*/5 * * * * *"
    }
    fn scope(&self) -> JobScope {
        JobScope::Node
    }

    async fn execute(&self, ctx: &JobContext) -> ProviderResult<JobResult> {
        let profile = ProfileBootstrap::get()
            .map_err(|error| SchedulerError::config_error(error.to_string()))?;
        let Some(evaluator) = &profile.evaluator else {
            return Ok(JobResult::success().with_message(
                "Evaluator supervisor is idle: the profile has no `evaluator` block",
            ));
        };
        let app = Arc::clone(
            ctx.app_context::<Arc<AppContext>>()
                .ok_or_else(|| SchedulerError::missing_context("AppContext"))?,
        );
        let supervisor = EvaluatorSupervisor::new(
            app.evaluation_repositories(),
            EvaluatorSupervisorConfig {
                docker: evaluator.docker.clone(),
                workspace_root: evaluator.workspace_root.clone(),
                environment: app.config().api_external_url.clone(),
                client_image: evaluator.client_image.clone(),
                relay_image: evaluator.relay_image.clone(),
                relay_control_network: evaluator.control_network.clone(),
                relay_upstream: app.config().api_external_url.clone(),
            },
        )?;
        let optimization = systemprompt_runtime::optimization::SkillOptimizationOrchestrator::new(
            app.managed_repository().as_ref().clone(),
            app.evaluation_repositories().as_ref().clone(),
        );
        let mut after = None;
        loop {
            let campaigns = app
                .evaluation_repositories()
                .campaigns
                .list(app.system_admin().id(), after.as_ref())
                .await
                .map_err(SchedulerError::Evaluation)?;
            let more = campaigns.len() > CampaignRepository::PAGE_SIZE;
            for campaign in campaigns {
                if let Err(error) = optimization
                    .advance(app.system_admin().id(), &ctx.actor().user_id, &campaign.id)
                    .await
                {
                    tracing::warn!(campaign_id = %campaign.id, error = %error, "Campaign iteration requires attention");
                }
                after = Some(campaign.id);
            }
            if !more {
                break;
            }
        }
        std::fs::create_dir_all(&supervisor.config.workspace_root)?;
        let processed = supervisor
            .run_once(&ctx.actor().user_id, &evaluator.worker_id)
            .await?;
        Ok(JobResult::success().with_stats(u64::from(processed), 0))
    }
}

systemprompt_provider_contracts::submit_job!(&EvaluationSupervisorJob);
