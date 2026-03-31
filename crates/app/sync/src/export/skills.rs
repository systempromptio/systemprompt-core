use super::escape_yaml;
use anyhow::Result;
use std::fs;
use std::path::Path;
use systemprompt_agent::models::Skill;

pub fn generate_skill_markdown(skill: &Skill) -> String {
    format!(
        "---\ndescription: \"{}\"\n---\n\n{}",
        escape_yaml(&skill.description),
        &skill.instructions
    )
}

pub fn generate_skill_config(skill: &Skill) -> String {
    let tags_yaml = if skill.tags.is_empty() {
        "[]".to_string()
    } else {
        skill
            .tags
            .iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"id: {}
name: "{}"
description: "{}"
enabled: {}
version: "1.0.0"
file: "SKILL.md"
assigned_agents:
  - content
tags:
{}"#,
        skill.id.as_str(),
        escape_yaml(&skill.name),
        escape_yaml(&skill.description),
        skill.enabled,
        tags_yaml
    )
}

pub fn export_skill_to_disk(skill: &Skill, base_path: &Path) -> Result<()> {
    let skill_dir_name = skill.id.as_str().replace('_', "-");
    let skill_dir = base_path.join(&skill_dir_name);
    fs::create_dir_all(&skill_dir)?;

    let skill_content = generate_skill_markdown(skill);
    fs::write(skill_dir.join("SKILL.md"), skill_content)?;

    let config_content = generate_skill_config(skill);
    fs::write(skill_dir.join("config.yaml"), config_content)?;

    Ok(())
}
