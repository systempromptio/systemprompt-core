use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use std::fs;
use std::path::Path;

use super::types::SkillEditOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_core_logging::CliService;
use systemprompt_models::ProfileBootstrap;

#[derive(Debug, Args)]
pub struct EditArgs {
    #[arg(help = "Skill name to edit")]
    pub name: Option<String>,

    #[arg(long = "set", value_name = "KEY=VALUE", help = "Set a configuration value")]
    pub set_values: Vec<String>,

    #[arg(long, help = "Enable the skill", conflicts_with = "disable")]
    pub enable: bool,

    #[arg(long, help = "Disable the skill", conflicts_with = "enable")]
    pub disable: bool,

    #[arg(long, help = "Update instructions")]
    pub instructions: Option<String>,

    #[arg(long, help = "File containing updated instructions")]
    pub instructions_file: Option<String>,
}

pub async fn execute(args: EditArgs, config: &CliConfig) -> Result<CommandResult<SkillEditOutput>> {
    let skills_path = get_skills_path()?;

    let name = resolve_input(args.name.clone(), "name", config, || {
        prompt_skill_selection(&skills_path)
    })?;

    let skill_dir = skills_path.join(&name);
    if !skill_dir.exists() {
        let alt_name = name.replace('_', "-");
        let alt_dir = skills_path.join(&alt_name);
        if !alt_dir.exists() {
            return Err(anyhow!("Skill '{}' not found", name));
        }
        return edit_skill(&alt_dir, &alt_name, &args).await;
    }

    edit_skill(&skill_dir, &name, &args).await
}

async fn edit_skill(
    skill_dir: &Path,
    name: &str,
    args: &EditArgs,
) -> Result<CommandResult<SkillEditOutput>> {
    let index_path = skill_dir.join("index.md");
    let skill_md_path = skill_dir.join("SKILL.md");

    let md_path = if index_path.exists() {
        index_path
    } else if skill_md_path.exists() {
        skill_md_path
    } else {
        return Err(anyhow!(
            "Skill '{}' has no index.md or SKILL.md file",
            name
        ));
    };

    let content = fs::read_to_string(&md_path)
        .with_context(|| format!("Failed to read {}", md_path.display()))?;

    let (mut frontmatter, instructions) = parse_markdown(&content)?;

    let mut changes = Vec::new();

    if args.enable {
        frontmatter.insert(
            serde_yaml::Value::String("enabled".to_string()),
            serde_yaml::Value::Bool(true),
        );
        changes.push("enabled: true".to_string());
    }

    if args.disable {
        frontmatter.insert(
            serde_yaml::Value::String("enabled".to_string()),
            serde_yaml::Value::Bool(false),
        );
        changes.push("enabled: false".to_string());
    }

    for set_value in &args.set_values {
        let parts: Vec<&str> = set_value.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(anyhow!(
                "Invalid --set format: '{}'. Expected key=value",
                set_value
            ));
        }
        apply_set_value(&mut frontmatter, parts[0], parts[1])?;
        changes.push(format!("{}: {}", parts[0], parts[1]));
    }

    let final_instructions = resolve_new_instructions(args, &instructions)?;
    if final_instructions != instructions {
        changes.push("instructions: updated".to_string());
    }

    if changes.is_empty() {
        return Err(anyhow!("No changes specified"));
    }

    CliService::info(&format!("Updating skill '{}'...", name));

    let new_content = rebuild_markdown(&frontmatter, &final_instructions)?;
    fs::write(&md_path, new_content)
        .with_context(|| format!("Failed to write {}", md_path.display()))?;

    CliService::success(&format!("Skill '{}' updated successfully", name));

    let output = SkillEditOutput {
        skill_id: name.replace('-', "_"),
        message: format!(
            "Skill '{}' updated successfully with {} change(s)",
            name,
            changes.len()
        ),
        changes,
    };

    Ok(CommandResult::text(output).with_title(&format!("Edit Skill: {}", name)))
}

fn get_skills_path() -> Result<std::path::PathBuf> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    Ok(std::path::PathBuf::from(profile.paths.skills()))
}

fn parse_markdown(content: &str) -> Result<(serde_yaml::Mapping, String)> {
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Err(anyhow!("Invalid frontmatter format"));
    }

    let frontmatter: serde_yaml::Mapping = serde_yaml::from_str(parts[1])
        .context("Invalid YAML frontmatter")?;

    Ok((frontmatter, parts[2].trim().to_string()))
}

fn rebuild_markdown(frontmatter: &serde_yaml::Mapping, instructions: &str) -> Result<String> {
    let yaml = serde_yaml::to_string(frontmatter).context("Failed to serialize frontmatter")?;

    Ok(format!("---\n{}---\n\n{}\n", yaml, instructions))
}

fn apply_set_value(
    frontmatter: &mut serde_yaml::Mapping,
    key: &str,
    value: &str,
) -> Result<()> {
    match key {
        "title" | "name" => {
            frontmatter.insert(
                serde_yaml::Value::String("title".to_string()),
                serde_yaml::Value::String(value.to_string()),
            );
        }
        "description" => {
            frontmatter.insert(
                serde_yaml::Value::String("description".to_string()),
                serde_yaml::Value::String(value.to_string()),
            );
        }
        "category" => {
            frontmatter.insert(
                serde_yaml::Value::String("category".to_string()),
                serde_yaml::Value::String(value.to_string()),
            );
        }
        "tags" | "keywords" => {
            let tags: Vec<serde_yaml::Value> = value
                .split(',')
                .map(|s| serde_yaml::Value::String(s.trim().to_string()))
                .collect();
            frontmatter.insert(
                serde_yaml::Value::String("keywords".to_string()),
                serde_yaml::Value::Sequence(tags),
            );
        }
        "enabled" => {
            let enabled = value.parse::<bool>().map_err(|_| {
                anyhow!("Invalid boolean value for enabled: '{}'. Use true or false", value)
            })?;
            frontmatter.insert(
                serde_yaml::Value::String("enabled".to_string()),
                serde_yaml::Value::Bool(enabled),
            );
        }
        _ => {
            return Err(anyhow!(
                "Unknown configuration key: '{}'. Supported: title, description, category, tags, enabled",
                key
            ));
        }
    }
    Ok(())
}

fn resolve_new_instructions(args: &EditArgs, current: &str) -> Result<String> {
    if let Some(i) = &args.instructions {
        return Ok(i.clone());
    }

    if let Some(file) = &args.instructions_file {
        let path = Path::new(file);
        return fs::read_to_string(path)
            .with_context(|| format!("Failed to read instructions file: {}", path.display()));
    }

    Ok(current.to_string())
}

fn prompt_skill_selection(skills_path: &Path) -> Result<String> {
    let mut skills: Vec<String> = Vec::new();

    if skills_path.exists() {
        for entry in fs::read_dir(skills_path)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let has_skill_file =
                path.join("index.md").exists() || path.join("SKILL.md").exists();

            if has_skill_file {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    skills.push(name.to_string());
                }
            }
        }
    }

    if skills.is_empty() {
        return Err(anyhow!("No skills found"));
    }

    skills.sort();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select skill to edit")
        .items(&skills)
        .default(0)
        .interact()
        .context("Failed to get skill selection")?;

    Ok(skills[selection].clone())
}
