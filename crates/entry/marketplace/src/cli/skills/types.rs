//! Output payloads and config parsing for the `skills` command group.
//!
//! Defines the list/detail response shapes and [`parse_skill_from_config`],
//! which loads a skill's YAML config plus its frontmatter-stripped instruction
//! body from disk. The config and the instruction file it names are read only
//! when they resolve inside the skill's own directory.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use systemprompt_manifest::{DiskSkillConfig, strip_frontmatter};
use systemprompt_models::path_confine::{locate_within, read_within};

pub use systemprompt_manifest::services::{SkillDetail, SkillSummary};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillListOutput {
    pub skills: Vec<SkillSummary>,
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
    let config_text = read_within(skill_dir, config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    let config: DiskSkillConfig = serde_yaml::from_str(&config_text)
        .with_context(|| format!("Invalid YAML in {}", config_path.display()))?;

    let instructions = match locate_within(skill_dir, &skill_dir.join(config.content_file()))? {
        Some(content_path) => strip_frontmatter(&read_within(skill_dir, &content_path)?),
        None => String::new(),
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
