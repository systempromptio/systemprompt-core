//! `skills list` subcommand.
//!
//! Scans the profile's skills directory for skill configs, rendering either a
//! filtered summary table or, when a skill name is given, a single-skill detail
//! card with an instructions preview.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::path::Path;
use systemprompt_identifiers::SkillId;
use systemprompt_models::SKILL_CONFIG_FILENAME;

use crate::CliConfig;
use crate::shared::{CommandOutput, truncate_with_ellipsis};

use super::types::{SkillDetailOutput, SkillListOutput, SkillSummary, parse_skill_from_config};

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(help = "Skill ID to show details (optional)")]
    pub name: Option<String>,

    #[arg(long, help = "Show only enabled skills")]
    pub enabled: bool,

    #[arg(long, help = "Show only disabled skills", conflicts_with = "enabled")]
    pub disabled: bool,
}

pub(super) fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let skills_path = get_skills_path()?;

    if let Some(name) = args.name {
        return show_skill_detail(&name, &skills_path);
    }

    let skills = scan_skills(&skills_path)?;

    let filtered: Vec<SkillSummary> = skills
        .into_iter()
        .filter(|s| {
            if args.enabled {
                s.enabled
            } else if args.disabled {
                !s.enabled
            } else {
                true
            }
        })
        .collect();

    let output = SkillListOutput { skills: filtered };

    Ok(CommandOutput::table_of(
        vec!["skill_id", "name", "enabled", "tags", "file_path"],
        &output.skills,
    )
    .with_title("Skills"))
}

fn get_skills_path() -> Result<std::path::PathBuf> {
    let profile = systemprompt_config::ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(profile.paths.skills()))
}

fn show_skill_detail(skill_name: &str, skills_path: &Path) -> Result<CommandOutput> {
    let skill_dir = skills_path.join(skill_name);

    if !skill_dir.exists() {
        return Err(anyhow!("Skill '{}' not found", skill_name));
    }

    let config_path = skill_dir.join(SKILL_CONFIG_FILENAME);

    if !config_path.exists() {
        return Err(anyhow!(
            "Skill '{}' has no {} file",
            skill_name,
            SKILL_CONFIG_FILENAME
        ));
    }

    let parsed = parse_skill_from_config(&config_path, &skill_dir)?;

    let instructions_preview = truncate_with_ellipsis(&parsed.instructions, 200);

    let output = SkillDetailOutput {
        skill_id: SkillId::new(skill_name),
        name: parsed.name.clone(),
        display_name: parsed.name,
        description: parsed.description,
        enabled: parsed.enabled,
        tags: parsed.tags,
        category: parsed.category,
        file_path: Some(config_path.to_string_lossy().to_string()),
        instructions_preview,
    };

    Ok(CommandOutput::card_value(
        format!("Skill: {}", skill_name),
        &output,
    ))
}

fn scan_skills(skills_path: &Path) -> Result<Vec<SkillSummary>> {
    if !skills_path.exists() {
        return Ok(Vec::new());
    }

    let mut skills = Vec::new();

    for entry in std::fs::read_dir(skills_path)? {
        let entry = entry?;
        let skill_path = entry.path();

        if !skill_path.is_dir() {
            continue;
        }

        let config_path = skill_path.join(SKILL_CONFIG_FILENAME);
        if !config_path.exists() {
            continue;
        }

        match parse_skill_from_config(&config_path, &skill_path) {
            Ok(parsed) => {
                let dir_name = skill_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow!("Invalid skill directory name"))?;

                skills.push(SkillSummary {
                    skill_id: SkillId::new(dir_name),
                    name: parsed.name.clone(),
                    display_name: parsed.name,
                    enabled: parsed.enabled,
                    tags: parsed.tags,
                    file_path: Some(config_path.to_string_lossy().to_string()),
                });
            },
            Err(e) => {
                tracing::warn!(
                    path = %skill_path.display(),
                    error = %e,
                    "Failed to parse skill"
                );
            },
        }
    }

    skills.sort_by(|a, b| a.skill_id.cmp(&b.skill_id));
    Ok(skills)
}
