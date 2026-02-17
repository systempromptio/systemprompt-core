use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::path::Path;

use crate::shared::CommandResult;
use crate::CliConfig;

use super::types::SkillDetailOutput;

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    #[arg(help = "Skill ID (directory name)")]
    pub name: String,
}

pub fn execute(args: &ShowArgs, _config: &CliConfig) -> Result<CommandResult<SkillDetailOutput>> {
    let skills_path = get_skills_path()?;
    show_skill_detail(&args.name, &skills_path)
}

fn get_skills_path() -> Result<std::path::PathBuf> {
    let profile = systemprompt_models::ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(profile.paths.skills()))
}

fn show_skill_detail(
    skill_id: &str,
    skills_path: &Path,
) -> Result<CommandResult<SkillDetailOutput>> {
    let skill_dir = skills_path.join(skill_id);

    if !skill_dir.exists() {
        return Err(anyhow!("Skill '{}' not found", skill_id));
    }

    let md_path = skill_dir.join("SKILL.md");

    if !md_path.exists() {
        return Err(anyhow!("Skill '{}' has no SKILL.md file", skill_id));
    }

    let parsed = parse_skill_markdown(&md_path)?;

    let instructions_preview = parsed.instructions.chars().take(200).collect::<String>()
        + if parsed.instructions.len() > 200 {
            "..."
        } else {
            ""
        };

    let output = SkillDetailOutput {
        skill_id: skill_id.to_string(),
        name: parsed.name,
        description: parsed.description,
        enabled: parsed.enabled,
        tags: parsed.tags,
        category: parsed.category,
        file_path: md_path.to_string_lossy().to_string(),
        instructions_preview,
    };

    Ok(CommandResult::card(output).with_title(format!("Skill: {}", skill_id)))
}

struct ParsedSkill {
    name: String,
    description: String,
    enabled: bool,
    tags: Vec<String>,
    category: Option<String>,
    instructions: String,
}

fn parse_skill_markdown(md_path: &Path) -> Result<ParsedSkill> {
    let content = std::fs::read_to_string(md_path)
        .with_context(|| format!("Failed to read {}", md_path.display()))?;

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Err(anyhow!(
            "Invalid frontmatter format in {}",
            md_path.display()
        ));
    }

    let frontmatter: serde_yaml::Value = serde_yaml::from_str(parts[1])
        .with_context(|| format!("Invalid YAML in {}", md_path.display()))?;

    let description = frontmatter
        .get("description")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing description in {}", md_path.display()))?
        .to_string();

    let instructions = parts[2].trim().to_string();

    let skill_dir = md_path.parent();
    let config_path = skill_dir.map(|d| d.join("config.yaml"));

    let (name, enabled, tags, category) = match config_path.filter(|p| p.exists()) {
        Some(cfg_path) => {
            let cfg_text = std::fs::read_to_string(&cfg_path)
                .with_context(|| format!("Failed to read {}", cfg_path.display()))?;
            let cfg: serde_yaml::Value = serde_yaml::from_str(&cfg_text)
                .with_context(|| format!("Invalid YAML in {}", cfg_path.display()))?;

            let cfg_name = cfg.get("name").and_then(|v| v.as_str()).map(String::from);
            let cfg_enabled = cfg.get("enabled").and_then(serde_yaml::Value::as_bool);
            let cfg_tags = cfg.get("tags").and_then(|v| v.as_sequence()).map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>()
            });
            let cfg_category = cfg
                .get("category")
                .and_then(|v| v.as_str())
                .map(String::from);

            (
                cfg_name.unwrap_or_else(|| description.clone()),
                cfg_enabled.unwrap_or(true),
                cfg_tags.unwrap_or_else(Vec::new),
                cfg_category,
            )
        },
        None => (description.clone(), true, Vec::new(), None),
    };

    Ok(ParsedSkill {
        name,
        description,
        enabled,
        tags,
        category,
        instructions,
    })
}
