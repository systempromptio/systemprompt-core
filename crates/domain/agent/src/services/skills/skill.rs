//! Disk-backed skill service: resolving the skills root, loading skill
//! definitions and metadata, broadcasting skill events, and recording skill
//! usage as execution steps.

use crate::repository::execution::ExecutionStepRepository;
use crate::services::ExecutionTrackingService;
use crate::services::a2a_server::streaming::webhook_client::{WebhookError, broadcast_agui_event};
use crate::services::shared::{AgentServiceError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::SkillId;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::{
    AgUiEventBuilder, DiskSkillConfig, SKILL_CONFIG_FILENAME, strip_frontmatter,
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
        })
    }

    /// Inject the execution-step repository so per-task `track_skill_usage`
    /// can run without round-tripping the database from the disk-only loader.
    pub fn with_execution_step_repo(mut self, repo: Arc<ExecutionStepRepository>) -> Self {
        self.execution_step_repo = Some(repo);
        self
    }

    pub async fn load_skill(&self, skill_id: &SkillId, ctx: &RequestContext) -> Result<String> {
        let loaded = load_disk_skill(self.skills_root.as_ref(), skill_id)?;

        tracing::info!(skill_id = %loaded.skill_id, "Loaded skill from disk");

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

        if let Some(task_id) = ctx.task_id() {
            tracing::info!(task_id = %task_id.as_str(), "Tracking skill usage for task");

            let Some(execution_step_repo) = self.execution_step_repo.as_ref() else {
                tracing::warn!(
                    "ExecutionStepRepository not injected; skill usage will not be tracked"
                );
                return Ok(loaded.instructions);
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

        Ok(loaded.instructions)
    }

    #[expect(
        clippy::unused_async,
        reason = "kept async to match sibling `SkillService::load_skill` (which does perform async I/O) so callers consume the whole surface with uniform `.await`"
    )]
    pub async fn list_skill_ids(&self) -> Result<Vec<String>> {
        list_enabled_skill_ids(self.skills_root.as_ref())
    }

    #[expect(
        clippy::unused_async,
        reason = "kept async to match sibling `SkillService::load_skill` (which does perform async I/O) so callers consume the whole surface with uniform `.await`"
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

struct LoadedDiskSkill {
    skill_id: SkillId,
    name: String,
    description: String,
    instructions: String,
}

fn resolve_skills_root() -> Result<PathBuf> {
    let profile = ProfileBootstrap::get().map_err(|e| {
        AgentServiceError::Internal(format!("Profile not initialized for SkillService: {e}"))
    })?;
    Ok(PathBuf::from(profile.paths.skills()))
}

fn load_disk_skill(skills_root: &Path, skill_id: &SkillId) -> Result<LoadedDiskSkill> {
    let id_str = skill_id.as_str();
    let skill_dir = skills_root.join(id_str);
    let config_path = skill_dir.join(SKILL_CONFIG_FILENAME);

    if !config_path.exists() {
        return Err(AgentServiceError::Internal(format!(
            "Skill not found on disk: {id_str} ({SKILL_CONFIG_FILENAME} missing at {})",
            config_path.display()
        )));
    }

    let config_text = std::fs::read_to_string(&config_path).map_err(|e| {
        AgentServiceError::Internal(format!("Failed to read {}: {e}", config_path.display()))
    })?;
    let config: DiskSkillConfig = serde_yaml::from_str(&config_text).map_err(|e| {
        AgentServiceError::Internal(format!("Invalid YAML in {}: {e}", config_path.display()))
    })?;

    let resolved_id = if config.id.as_str().is_empty() {
        skill_id.clone()
    } else {
        config.id.clone()
    };

    let content_path = skill_dir.join(config.content_file());
    let instructions = if content_path.exists() {
        let raw = std::fs::read_to_string(&content_path).map_err(|e| {
            AgentServiceError::Internal(format!("Failed to read {}: {e}", content_path.display()))
        })?;
        strip_frontmatter(&raw)
    } else {
        String::new()
    };

    let name = if config.name.is_empty() {
        id_str.to_owned()
    } else {
        config.name
    };

    Ok(LoadedDiskSkill {
        skill_id: resolved_id,
        name,
        description: config.description,
        instructions,
    })
}

fn list_enabled_skill_ids(skills_root: &Path) -> Result<Vec<String>> {
    if !skills_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut ids: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(skills_root).map_err(|e| {
        AgentServiceError::Internal(format!(
            "Failed to read skills dir {}: {e}",
            skills_root.display()
        ))
    })? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let config_path = path.join(SKILL_CONFIG_FILENAME);
        if !config_path.exists() {
            continue;
        }
        let config_text = match std::fs::read_to_string(&config_path) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(path = %config_path.display(), error = %e, "skill: read failed; skipping");
                continue;
            },
        };
        let config: DiskSkillConfig = match serde_yaml::from_str(&config_text) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %config_path.display(), error = %e, "skill: invalid YAML; skipping");
                continue;
            },
        };
        if !config.enabled {
            continue;
        }
        let dir_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
            AgentServiceError::Internal(format!(
                "Invalid skill dir entry under {}",
                skills_root.display()
            ))
        })?;
        let id = if config.id.as_str().is_empty() {
            dir_name.to_owned()
        } else {
            config.id.as_str().to_owned()
        };
        ids.push(id);
    }
    ids.sort();
    Ok(ids)
}

async fn broadcast_skill_event(
    ctx: &RequestContext,
    event: systemprompt_models::AgUiEvent,
) -> std::result::Result<usize, WebhookError> {
    broadcast_agui_event(ctx.user_id(), event, ctx.auth_token().as_str()).await
}
