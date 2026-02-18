use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;

use crate::models::a2a::AgentSkill;

const SKILL_FILENAME: &str = "SKILL.md";
const CONFIG_FILENAME: &str = "config.yaml";

#[derive(Debug, serde::Deserialize)]
struct SkillConfig {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    examples: Option<Vec<String>>,
    #[serde(default)]
    input_modes: Option<Vec<String>>,
    #[serde(default)]
    output_modes: Option<Vec<String>>,
}

pub fn load_skill_from_disk(skills_path: &Path, skill_id: &str) -> Result<AgentSkill> {
    let skill_dir = skills_path.join(skill_id);
    let skill_path = skill_dir.join(SKILL_FILENAME);

    if !skill_path.exists() {
        anyhow::bail!(
            "Skill directory or {} not found: {}",
            SKILL_FILENAME,
            skill_path.display()
        );
    }

    let content = fs::read_to_string(&skill_path)?;
    let description = extract_description(&content);

    let config_path = skill_dir.join(CONFIG_FILENAME);
    let config = if config_path.exists() {
        let config_text = fs::read_to_string(&config_path)?;
        serde_yaml::from_str::<SkillConfig>(&config_text)
            .map_err(|e| anyhow!("Failed to parse {}: {}", CONFIG_FILENAME, e))?
    } else {
        SkillConfig {
            name: None,
            description: None,
            tags: Vec::new(),
            examples: None,
            input_modes: None,
            output_modes: None,
        }
    };

    Ok(AgentSkill {
        id: skill_id.to_string(),
        name: config.name.unwrap_or_else(|| skill_id.to_string()),
        description: config
            .description
            .or(description)
            .unwrap_or_else(|| format!("{skill_id} skill")),
        tags: config.tags,
        examples: config.examples,
        input_modes: config.input_modes,
        output_modes: config.output_modes,
        security: None,
    })
}

fn extract_description(content: &str) -> Option<String> {
    if !content.starts_with("---") {
        return None;
    }

    let content_after_start = &content[3..];
    let yaml_content = content_after_start
        .find("\n---")
        .map(|pos| &content_after_start[..pos])?;

    let yaml: serde_yaml::Value = serde_yaml::from_str(yaml_content)
        .map_err(|e| {
            tracing::warn!(error = %e, "Failed to parse skill frontmatter YAML");
            e
        })
        .ok()?;

    yaml.get("description")
        .and_then(|v| v.as_str())
        .map(String::from)
}
