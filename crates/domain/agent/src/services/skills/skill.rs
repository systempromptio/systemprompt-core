//! Disk-backed skill service: resolving the skills root, loading skill
//! definitions and metadata, broadcasting skill events, and recording skill
//! usage as execution steps.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::repository::execution::ExecutionStepRepository;
use crate::services::ExecutionTrackingService;
use crate::services::a2a_server::streaming::webhook_client::{WebhookError, broadcast_agui_event};
use crate::services::shared::{AgentServiceError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::SkillId;
use systemprompt_loader::ServicesRootBootstrap;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::{
    AgUiEventBuilder, DiskSkillConfig, SKILL_CONFIG_FILENAME, strip_frontmatter,
};

#[path = "disk.rs"]
mod disk;
use disk::{
    LoadedDiskSkill, broadcast_skill_event, list_enabled_skill_ids, load_disk_skill,
    resolve_skills_root,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub skill_id: SkillId,
    pub name: String,
}

#[derive(Clone)]
pub struct SkillService {
    skills_root: Arc<PathBuf>,
    execution_step_repo: Option<Arc<ExecutionStepRepository>>,
    managed: Option<systemprompt_traits::DynManagedSkillResolver>,
}

impl std::fmt::Debug for SkillService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillService")
            .field("skills_root", &self.skills_root)
            .field(
                "execution_step_repo",
                &self
                    .execution_step_repo
                    .as_ref()
                    .map_or("<None>", |_| "<ExecutionStepRepository>"),
            )
            .finish()
    }
}

impl SkillService {
    pub fn new() -> Result<Self> {
        let skills_root = resolve_skills_root()?;
        Ok(Self {
            skills_root: Arc::new(skills_root),
            execution_step_repo: None,
            managed: None,
        })
    }

    /// Enable fail-closed managed resolution for runtime loads. Disk remains
    /// available only for keys that are not registered as managed resources.
    pub fn with_managed_resolver(
        mut self,
        resolver: systemprompt_traits::DynManagedSkillResolver,
    ) -> Self {
        self.managed = Some(resolver);
        self
    }

    pub fn with_execution_step_repo(mut self, repo: Arc<ExecutionStepRepository>) -> Self {
        self.execution_step_repo = Some(repo);
        self
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

        tracing::info!(skill_id = %loaded.skill_id, "Broadcasting skill_loaded event");

        if let Err(e) = broadcast_skill_event(ctx, event).await {
            tracing::error!(error = %e, "Failed to broadcast skill_loaded");
        }

        self.track_skill_usage(&loaded, ctx).await;

        Ok(loaded.instructions)
    }

    async fn resolve_runtime_skill(
        &self,
        skill_id: &SkillId,
        owner: &systemprompt_identifiers::UserId,
    ) -> Result<LoadedDiskSkill> {
        if let Some(resolver) = &self.managed {
            match resolver.resolve_skill(owner, skill_id.as_str()).await {
                Ok(Some(skill)) => {
                    return Ok(LoadedDiskSkill {
                        skill_id: SkillId::new(skill.id),
                        name: skill.name,
                        description: skill.description,
                        instructions: skill.instructions,
                    });
                },
                Ok(None) => {},
                Err(error) => {
                    return Err(AgentServiceError::Internal(format!(
                        "Managed skill {} is unavailable: {error}",
                        skill_id.as_str()
                    )));
                },
            }
        }
        load_disk_skill(self.skills_root.as_ref(), skill_id)
    }

    async fn track_skill_usage(&self, loaded: &LoadedDiskSkill, ctx: &RequestContext) {
        let Some(task_id) = ctx.task_id() else {
            tracing::warn!("No task_id in context - skill usage not tracked");
            return;
        };

        tracing::info!(task_id = %task_id.as_str(), "Tracking skill usage for task");

        let Some(execution_step_repo) = self.execution_step_repo.as_ref() else {
            tracing::warn!("ExecutionStepRepository not injected; skill usage will not be tracked");
            return;
        };

        let tracking = ExecutionTrackingService::new(Arc::clone(execution_step_repo));
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
                if let Err(e) = broadcast_skill_event(ctx, step_event).await {
                    tracing::error!(error = %e, "Failed to broadcast execution_step");
                }
            },
            Err(e) => {
                tracing::error!(error = %e, "Failed to track skill usage");
            },
        }
    }

    #[expect(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "async signature kept so the call site can stay uniform with load_skill, which \
                  is genuinely async"
    )]
    pub async fn list_skill_ids(&self) -> Result<Vec<String>> {
        list_enabled_skill_ids(self.skills_root.as_ref())
    }

    #[expect(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "async signature kept so the call site can stay uniform with load_skill, which \
                  is genuinely async"
    )]
    pub async fn load_skill_metadata(&self, skill_id: &SkillId) -> Result<SkillMetadata> {
        let loaded = load_disk_skill(self.skills_root.as_ref(), skill_id)?;
        tracing::info!(skill_id = %loaded.skill_id, "Loaded skill metadata from disk");
        Ok(SkillMetadata {
            skill_id: loaded.skill_id,
            name: loaded.name,
        })
    }
}
