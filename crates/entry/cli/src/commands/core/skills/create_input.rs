use anyhow::{Context, Result, anyhow};
use dialoguer::Input;
use dialoguer::theme::ColorfulTheme;
use std::fs;
use std::path::Path;

use crate::CliConfig;

pub(super) fn validate_skill_name(name: &str) -> Result<()> {
    if name.len() < 3 || name.len() > 50 {
        return Err(anyhow!("Skill name must be between 3 and 50 characters"));
    }

    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(anyhow!(
            "Skill name must be lowercase alphanumeric with underscores only"
        ));
    }

    Ok(())
}

fn normalize_skill_name(name: &str) -> String {
    name.replace('-', "_").to_lowercase()
}

pub(super) fn check_normalized_conflicts(name: &str, skills_dir: &Path) -> Result<()> {
    let normalized_name = normalize_skill_name(name);

    if !skills_dir.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(skills_dir)
        .with_context(|| format!("Failed to read skills directory: {}", skills_dir.display()))?;

    for entry in entries.filter_map(std::result::Result::ok) {
        if !entry.path().is_dir() {
            continue;
        }

        let existing_name = entry.file_name().to_string_lossy().to_string();
        let existing_normalized = normalize_skill_name(&existing_name);

        if existing_name == name {
            continue;
        }

        if existing_normalized == normalized_name {
            return Err(anyhow!(
                "Skill '{}' conflicts with existing skill '{}' (same normalized name: '{}'). Use \
                 consistent naming to avoid confusion.",
                name,
                existing_name,
                normalized_name
            ));
        }
    }

    Ok(())
}

pub(super) fn title_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn resolve_instructions(
    instructions: Option<&str>,
    instructions_file: Option<&str>,
    config: &CliConfig,
) -> Result<String> {
    if let Some(i) = instructions {
        return Ok(i.to_string());
    }

    if let Some(file) = instructions_file {
        let path = Path::new(file);
        return fs::read_to_string(path)
            .with_context(|| format!("Failed to read instructions file: {}", path.display()));
    }

    if config.is_interactive() {
        return prompt_instructions();
    }

    Ok(String::new())
}

pub(super) fn build_skill_markdown(description: &str, instructions: &str) -> String {
    format!(
        "---\ndescription: \"{description}\"\n---\n\n{instructions}\n",
        description = description,
        instructions = instructions
    )
}

pub(super) fn build_skill_config(
    name: &str,
    display_name: &str,
    description: &str,
    enabled: bool,
    tags: &[String],
) -> String {
    let tags_yaml = if tags.is_empty() {
        "[]".to_string()
    } else {
        tags.iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"id: {name}
name: "{display_name}"
description: "{description}"
enabled: {enabled}
version: "1.0.0"
file: "SKILL.md"
assigned_agents:
  - content
tags:
{tags_yaml}"#,
        name = name,
        display_name = display_name,
        description = description,
        enabled = enabled,
        tags_yaml = tags_yaml
    )
}

pub(super) fn prompt_name() -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Skill name (slug)")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.len() < 3 {
                return Err("Name must be at least 3 characters");
            }
            if !input
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                return Err("Name must be lowercase alphanumeric with underscores only");
            }
            Ok(())
        })
        .interact_text()
        .context("Failed to get skill name")
}

pub(super) fn prompt_display_name(default: &str) -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Display name")
        .default(title_case(default))
        .interact_text()
        .context("Failed to get display name")
}

pub(super) fn prompt_description() -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Description")
        .allow_empty(true)
        .interact_text()
        .context("Failed to get description")
}

fn prompt_instructions() -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Instructions (single line, or use --instructions-file)")
        .allow_empty(true)
        .interact_text()
        .context("Failed to get instructions")
}
