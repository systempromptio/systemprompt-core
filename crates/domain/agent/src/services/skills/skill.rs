//! Skill service backed by the installation's managed authority.
//!
//! A skill resolves through the managed resolver first and falls back to the
//! disk catalogue only for keys that are not managed; loads broadcast skill
//! events and record usage as execution steps.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::repository::execution::ExecutionStepRepository;
use crate::services::ExecutionTrackingService;
use crate::services::shared::{AgentServiceError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_identifiers::{SkillId, UserId};
use systemprompt_models::AgUiEventBuilder;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_traits::{DynManagedSkillResolver, SkillResolution};

use super::disk::{LoadedDiskSkill, load_disk_skill, resolve_skills_root};
use crate::services::a2a_server::streaming::webhook_client::{
    DynWebhookBroadcaster, WebhookContext,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub skill_id: SkillId,
    pub name: String,
}

#[derive(Clone)]
pub struct SkillService {
    skills_root: Arc<PathBuf>,
    execution_step_repo: Arc<ExecutionStepRepository>,
    managed: DynManagedSkillResolver,
    webhooks: DynWebhookBroadcaster,
}

impl std::fmt::Debug for SkillService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillService")
            .field("skills_root", &self.skills_root)
            .field("managed", &self.managed)
            .finish_non_exhaustive()
    }
}

impl SkillService {
    pub fn new(
        managed: DynManagedSkillResolver,
        execution_step_repo: Arc<ExecutionStepRepository>,
        webhooks: DynWebhookBroadcaster,
    ) -> Result<Self> {
        let skills_root = resolve_skills_root()?;
        Ok(Self {
            skills_root: Arc::new(skills_root),
            execution_step_repo,
            managed,
            webhooks,
        })
    }

    pub async fn load_skill(&self, skill_id: &SkillId, ctx: &RequestContext) -> Result<String> {
        let loaded = self.resolve_runtime_skill(skill_id, ctx.user_id()).await?;

        tracing::info!(skill_id = %loaded.skill_id, "Loaded resolved skill");

        let event = AgUiEventBuilder::skill_loaded(
            loaded.skill_id.clone(),
            loaded.name.clone(),
            Some(loaded.description.clone()),
            ctx.task_id().cloned(),
        );

        let webhooks = WebhookContext::for_request(Arc::clone(&self.webhooks), ctx);
        if let Err(e) = webhooks.broadcast_agui(event).await {
            tracing::error!(error = %e, "Failed to broadcast skill_loaded");
        }

        self.track_skill_usage(&loaded, ctx).await;

        Ok(loaded.instructions)
    }

    pub async fn load_skill_metadata(
        &self,
        skill_id: &SkillId,
        owner: &UserId,
    ) -> Result<SkillMetadata> {
        let loaded = self.resolve_runtime_skill(skill_id, owner).await?;
        Ok(SkillMetadata {
            skill_id: loaded.skill_id,
            name: loaded.name,
        })
    }

    async fn resolve_runtime_skill(
        &self,
        skill_id: &SkillId,
        owner: &UserId,
    ) -> Result<LoadedDiskSkill> {
        match self.managed.resolve_skill(owner, skill_id.as_str()).await {
            Ok(SkillResolution::Published(skill)) => Ok(LoadedDiskSkill {
                skill_id: skill.id,
                name: skill.name,
                description: skill.description,
                instructions: skill.instructions,
            }),
            Ok(SkillResolution::NotManaged) => load_disk_skill(self.skills_root.as_ref(), skill_id),
            Ok(SkillResolution::Withheld(reason)) => Err(AgentServiceError::SkillWithheld {
                skill_id: skill_id.clone(),
                reason: reason.as_str(),
            }),
            Err(error) => Err(AgentServiceError::SkillSource {
                skill_id: skill_id.clone(),
                source: error,
            }),
        }
    }

    async fn track_skill_usage(&self, loaded: &LoadedDiskSkill, ctx: &RequestContext) {
        let Some(task_id) = ctx.task_id() else {
            tracing::warn!("No task_id in context - skill usage not tracked");
            return;
        };

        let tracking = ExecutionTrackingService::new(Arc::clone(&self.execution_step_repo));
        match tracking
            .track_skill_usage(
                task_id.clone(),
                loaded.skill_id.clone(),
                loaded.name.clone(),
            )
            .await
        {
            Ok(step) => {
                tracing::info!(step_id = %step.step_id.as_str(), "Skill usage tracked");

                let step_event = AgUiEventBuilder::execution_step(step, ctx.context_id().clone());
                let webhooks = WebhookContext::for_request(Arc::clone(&self.webhooks), ctx);
                if let Err(e) = webhooks.broadcast_agui(step_event).await {
                    tracing::error!(error = %e, "Failed to broadcast execution_step");
                }
            },
            Err(e) => {
                tracing::error!(error = %e, "Failed to track skill usage");
            },
        }
    }
}
