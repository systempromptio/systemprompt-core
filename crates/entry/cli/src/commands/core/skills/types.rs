//! Output payloads and config parsing for the `skills` command group.
//!
//! Defines the list/detail response shapes and [`parse_skill_from_config`],
//! which loads a skill's YAML config plus its frontmatter-stripped instruction
//! body from disk.

use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use systemprompt_models::{DiskSkillConfig, strip_frontmatter};

pub use systemprompt_models::services::{SkillDetail as SkillDetailOutput, SkillSummary};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillListOutput {
    pub skills: Vec<SkillSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ListOrDetail {
    List(SkillListOutput),
    Detail(SkillDetailOutput),
}

#[derive(Debug)]
pub struct ParsedSkill {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub instructions: String,
}

pub fn parse_skill_from_config(config_path: &Path, skill_dir: &Path) -> Result<ParsedSkill> {
    let config_text = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    let config: DiskSkillConfig = serde_yaml::from_str(&config_text)
        .with_context(|| format!("Invalid YAML in {}", config_path.display()))?;

    let content_path = skill_dir.join(config.content_file());

    let instructions = if content_path.exists() {
        let raw = std::fs::read_to_string(&content_path)
            .with_context(|| format!("Failed to read {}", content_path.display()))?;
        strip_frontmatter(&raw)
    } else {
        String::new()
    };

    Ok(ParsedSkill {
        name: config.name,
        description: config.description,
        enabled: config.enabled,
        tags: config.tags,
        category: config.category,
        instructions,
    })
}
