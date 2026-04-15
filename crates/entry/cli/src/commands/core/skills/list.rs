use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::path::Path;
use systemprompt_models::SKILL_CONFIG_FILENAME;

use crate::CliConfig;
use crate::shared::{CommandResult, truncate_with_ellipsis};

use super::types::{
    ListOrDetail, SkillDetailOutput, SkillListOutput, SkillSummary, parse_skill_from_config,
};

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    #[arg(help = "Skill ID to show details (optional)")]
    pub name: Option<String>,

    #[arg(long, help = "Show only enabled skills")]
    pub enabled: bool,

    #[arg(long, help = "Show only disabled skills", conflicts_with = "enabled")]
    pub disabled: bool,
}

pub fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandResult<ListOrDetail>> {
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

    Ok(CommandResult::table(ListOrDetail::List(output))
        .with_title("Skills")
        .with_columns(vec![
            "skill_id".to_string(),
            "name".to_string(),
            "enabled".to_string(),
            "tags".to_string(),
            "file_path".to_string(),
        ]))
}

fn get_skills_path() -> Result<std::path::PathBuf> {
    let profile = systemprompt_models::ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(profile.paths.skills()))
}

fn show_skill_detail(skill_name: &str, skills_path: &Path) -> Result<CommandResult<ListOrDetail>> {
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
        skill_id: skill_name.to_string(),
        name: parsed.name.clone(),
        display_name: parsed.name,
        description: parsed.description,
        enabled: parsed.enabled,
        tags: parsed.tags,
        category: parsed.category,
        file_path: Some(config_path.to_string_lossy().to_string()),
        instructions_preview,
    };

    Ok(CommandResult::card(ListOrDetail::Detail(output))
        .with_title(format!("Skill: {}", skill_name)))
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
                    skill_id: dir_name.to_string(),
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
