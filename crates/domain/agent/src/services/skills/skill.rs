use crate::repository::content::SkillRepository;
use crate::repository::execution::ExecutionStepRepository;
use crate::services::a2a_server::streaming::webhook_client::{broadcast_agui_event, WebhookError};
use crate::services::ExecutionTrackingService;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SkillId, TaskId};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::AgUiEventBuilder;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub skill_id: SkillId,
    pub name: String,
}

#[derive(Clone)]
pub struct SkillService {
    skill_repo: Arc<SkillRepository>,
    db_pool: DbPool,
}

impl std::fmt::Debug for SkillService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillService")
            .field("skill_repo", &"<SkillRepository>")
            .finish()
    }
}

impl SkillService {
    pub fn new(db_pool: &DbPool) -> Result<Self> {
        Ok(Self {
            skill_repo: Arc::new(SkillRepository::new(db_pool)?),
            db_pool: db_pool.clone(),
        })
    }

    pub async fn load_skill(&self, skill_id: &str, ctx: &RequestContext) -> Result<String> {
        let skill_id_typed = SkillId::new(skill_id);

        let skill = self
            .skill_repo
            .get_by_skill_id(&skill_id_typed)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Skill not found in database: {} (ensure skill is synced via \
                     SkillIngestionService)",
                    skill_id
                )
            })?;

        tracing::info!(skill_id = %skill.skill_id, "Loaded skill");

        let event = AgUiEventBuilder::skill_loaded(
            skill.skill_id.clone(),
            skill.name.clone(),
            Some(skill.description.clone()),
            ctx.task_id().cloned(),
        );

        tracing::info!(skill_id = %skill.skill_id, "Broadcasting skill_loaded event");

        if let Err(e) = broadcast_skill_event(ctx, event).await {
            tracing::error!(error = %e, "Failed to broadcast skill_loaded");
        }

        if let Some(task_id) = ctx.task_id() {
            tracing::info!(task_id = %task_id.as_str(), "Tracking skill usage for task");

            let execution_step_repo = match ExecutionStepRepository::new(&self.db_pool) {
                Ok(repo) => Arc::new(repo),
                Err(e) => {
                    tracing::error!(error = %e, "Failed to create ExecutionStepRepository");
                    return Ok(skill.instructions);
                },
            };
            let tracking = ExecutionTrackingService::new(execution_step_repo);
            match tracking
                .track_skill_usage(
                    TaskId::new(task_id.as_str()),
                    skill.skill_id.clone(),
                    skill.name.clone(),
                )
                .await
            {
                Ok(step) => {
                    tracing::info!(step_id = %step.step_id.as_str(), "Skill usage tracked");

                    let step_event =
                        AgUiEventBuilder::execution_step(step, ctx.context_id().clone());
                    if let Err(e) = broadcast_skill_event(ctx, step_event).await {
                        tracing::error!(error = %e, "Failed to broadcast execution_step");
                    }
                },
                Err(e) => {
                    tracing::error!(error = %e, "Failed to track skill usage");
                },
            }
        } else {
            tracing::warn!("No task_id in context - skill usage not tracked");
        }

        Ok(skill.instructions)
    }

    pub async fn list_skill_ids(&self) -> Result<Vec<String>> {
        let skills = self.skill_repo.list_enabled().await?;
        Ok(skills.into_iter().map(|s| s.skill_id.to_string()).collect())
    }

    pub async fn load_skill_metadata(&self, skill_id: &str) -> Result<SkillMetadata> {
        let skill_id_typed = SkillId::new(skill_id);

        let skill = self
            .skill_repo
            .get_by_skill_id(&skill_id_typed)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Skill not found in database: {} (ensure skill is synced via \
                     SkillIngestionService)",
                    skill_id
                )
            })?;

        tracing::info!(skill_id = %skill.skill_id, "Loaded skill metadata");

        Ok(SkillMetadata {
            skill_id: skill.skill_id,
            name: skill.name,
        })
    }
}

async fn broadcast_skill_event(
    ctx: &RequestContext,
    event: systemprompt_models::AgUiEvent,
) -> Result<usize, WebhookError> {
    broadcast_agui_event(ctx.user_id().as_str(), event, ctx.auth_token().as_str()).await
}
