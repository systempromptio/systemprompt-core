use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;

use crate::models::a2a::AgentSkill;

pub fn load_skill_from_disk(skills_path: &Path, skill_id: &str) -> Result<AgentSkill> {
    let skill_dir = skills_path.join(skill_id);
    let index_path = skill_dir.join("index.md");

    if !index_path.exists() {
        anyhow::bail!(
            "Skill directory or index.md not found: {}",
            index_path.display()
        );
    }

    let content = fs::read_to_string(&index_path)?;
    let frontmatter = parse_skill_frontmatter(&content)?;

    Ok(AgentSkill {
        id: skill_id.to_string(),
        name: frontmatter.title.unwrap_or_else(|| skill_id.to_string()),
        description: frontmatter.description.unwrap_or_else(String::new),
        tags: frontmatter.keywords.unwrap_or_else(Vec::new),
        examples: frontmatter.examples,
        input_modes: frontmatter.input_modes,
        output_modes: frontmatter.output_modes,
        security: None,
    })
}

#[derive(Debug, Default)]
struct SkillFrontmatter {
    title: Option<String>,
    description: Option<String>,
    keywords: Option<Vec<String>>,
    examples: Option<Vec<String>>,
    input_modes: Option<Vec<String>>,
    output_modes: Option<Vec<String>>,
}

fn parse_skill_frontmatter(content: &str) -> Result<SkillFrontmatter> {
    if !content.starts_with("---") {
        return Ok(SkillFrontmatter::default());
    }

    let content_after_start = &content[3..];
    let yaml_content = match content_after_start.find("\n---") {
        Some(pos) => &content_after_start[..pos],
        None => return Ok(SkillFrontmatter::default()),
    };

    let yaml: serde_yaml::Value = serde_yaml::from_str(yaml_content)
        .map_err(|e| anyhow!("Failed to parse skill frontmatter: {}", e))?;

    let title = yaml.get("title").and_then(|v| v.as_str()).map(String::from);
    let description = yaml
        .get("description")
        .and_then(|v| v.as_str())
        .map(String::from);

    let keywords = yaml.get("keywords").and_then(|v| {
        v.as_str()
            .map(|s| {
                s.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .or_else(|| {
                v.as_sequence().map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
            })
    });

    let examples = yaml.get("examples").and_then(|v| {
        v.as_sequence().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
    });

    let input_modes = yaml.get("input_modes").and_then(|v| {
        v.as_sequence().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
    });

    let output_modes = yaml.get("output_modes").and_then(|v| {
        v.as_sequence().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
    });

    Ok(SkillFrontmatter {
        title,
        description,
        keywords,
        examples,
        input_modes,
        output_modes,
    })
}
