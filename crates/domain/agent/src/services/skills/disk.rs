//! Filesystem skill authoring helpers and event delivery.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::{AgentServiceError, Result};
use std::path::{Path, PathBuf};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::SkillId;
use systemprompt_loader::ServicesRootBootstrap;
use systemprompt_models::{DiskSkillConfig, SKILL_CONFIG_FILENAME, strip_frontmatter};

pub(super) struct LoadedDiskSkill {
    pub(super) skill_id: SkillId,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) instructions: String,
}

pub(super) fn resolve_skills_root() -> Result<PathBuf> {
    let profile = ProfileBootstrap::get().map_err(|e| {
        AgentServiceError::Internal(format!("Profile not initialized for SkillService: {e}"))
    })?;
    Ok(ServicesRootBootstrap::active_path_or(
        &profile.paths.services,
        "skills",
    ))
}

pub(super) fn load_disk_skill(skills_root: &Path, skill_id: &SkillId) -> Result<LoadedDiskSkill> {
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
