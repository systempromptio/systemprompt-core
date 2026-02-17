use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::path::Path;

use crate::shared::CommandResult;
use crate::CliConfig;

use super::types::{ListOrDetail, SkillDetailOutput, SkillListOutput, SkillSummary};

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

fn show_skill_detail(skill_id: &str, skills_path: &Path) -> Result<CommandResult<ListOrDetail>> {
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

    Ok(
        CommandResult::card(ListOrDetail::Detail(output))
            .with_title(format!("Skill: {}", skill_id)),
    )
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

        let md_path = skill_path.join("SKILL.md");
        if !md_path.exists() {
            continue;
        }

        match parse_skill_markdown(&md_path) {
            Ok(parsed) => {
                let dir_name = skill_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                skills.push(SkillSummary {
                    skill_id: dir_name.to_string(),
                    name: parsed.name,
                    enabled: parsed.enabled,
                    tags: parsed.tags,
                    file_path: md_path.to_string_lossy().to_string(),
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
