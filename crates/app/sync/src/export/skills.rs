use super::escape_yaml;
use anyhow::Result;
use std::fs;
use std::path::Path;
use systemprompt_agent::models::Skill;

pub fn generate_skill_markdown(skill: &Skill) -> String {
    let tags_str = skill.tags.join(", ");
    let category = skill
        .category_id
        .as_ref()
        .map(|c| c.as_str())
        .unwrap_or("skills");

    format!(
        r#"---
title: "{}"
slug: "{}"
description: "{}"
author: "systemprompt"
published_at: "{}"
type: "skill"
category: "{}"
keywords: "{}"
---

{}"#,
        escape_yaml(&skill.name),
        skill.skill_id.as_str().replace('_', "-"),
        escape_yaml(&skill.description),
        skill.created_at.format("%Y-%m-%d"),
        category,
        tags_str,
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
file: "index.md"
assigned_agents:
  - content
tags:
{}"#,
        skill.skill_id.as_str(),
        escape_yaml(&skill.name),
        escape_yaml(&skill.description),
        skill.enabled,
        tags_yaml
    )
}

pub fn export_skill_to_disk(skill: &Skill, base_path: &Path) -> Result<()> {
    let skill_dir_name = skill.skill_id.as_str().replace('_', "-");
    let skill_dir = base_path.join(&skill_dir_name);
    fs::create_dir_all(&skill_dir)?;

    let index_content = generate_skill_markdown(skill);
    fs::write(skill_dir.join("index.md"), index_content)?;

    let config_content = generate_skill_config(skill);
    fs::write(skill_dir.join("config.yaml"), config_content)?;

    Ok(())
}
