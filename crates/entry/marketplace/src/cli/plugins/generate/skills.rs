//! Skill markdown generation for plugin bundles.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use systemprompt_manifest::{PluginConfig, strip_frontmatter};
use systemprompt_models::path_confine::{
    entries_within, locate_within, read_within, single_segment,
};
use systemprompt_models::plugin::{ComponentFilter, ComponentSource};

#[derive(Debug, Deserialize)]
struct SkillConfigHeader {
    name: Option<String>,
    description: Option<String>,
}

pub fn generate_skills(
    plugin: &PluginConfig,
    skills_path: &Path,
    output_dir: &Path,
    files_generated: &mut Vec<String>,
) -> Result<()> {
    let resolved_skills = resolve_skills(plugin, skills_path)?;

    for (skill_id, skill_dir) in &resolved_skills {
        let kebab_name = skill_id.replace('_', "-");
        let output_skill_dir = output_dir.join("skills").join(&kebab_name);
        std::fs::create_dir_all(&output_skill_dir)?;

        let skill_md_content = build_skill_md(skill_id, skill_dir)?;
        let skill_md_path = output_skill_dir.join("SKILL.md");
        std::fs::write(&skill_md_path, skill_md_content)?;
        files_generated.push(skill_md_path.to_string_lossy().to_string());
    }

    Ok(())
}

fn resolve_skills(plugin: &PluginConfig, skills_path: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut resolved = Vec::new();

    if plugin.skills.source == ComponentSource::Explicit {
        for skill_id in &plugin.skills.include {
            let segment = single_segment("skill id", skill_id.as_str())?;
            if let Some(skill_dir) = locate_within(skills_path, &skills_path.join(segment))? {
                resolved.push((skill_id.clone(), skill_dir));
            }
        }
        return Ok(resolved);
    }

    if !skills_path.exists() {
        return Ok(resolved);
    }

    for entry in entries_within(skills_path, skills_path)? {
        if !entry.is_dir {
            continue;
        }
        let path = entry.path;

        let skill_id = entry.name.to_string_lossy().to_string();

        if plugin.skills.exclude.contains(&skill_id) {
            continue;
        }

        if plugin.skills.filter == Some(ComponentFilter::Enabled)
            && let Some(config_path) = locate_within(&path, &path.join("config.yaml"))?
        {
            let cfg_text = read_within(&path, &config_path)?;
            let cfg: serde_yaml::Value = serde_yaml::from_str(&cfg_text)?;
            let enabled = cfg
                .get("enabled")
                .and_then(serde_yaml::Value::as_bool)
                .unwrap_or(true);
            if !enabled {
                continue;
            }
        }

        resolved.push((skill_id, path));
    }

    resolved.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(resolved)
}

fn build_skill_md(skill: &str, skill_dir: &Path) -> Result<String> {
    let index_md = locate_within(skill_dir, &skill_dir.join("index.md"))?;
    let skill_md_path = locate_within(skill_dir, &skill_dir.join("SKILL.md"))?;

    let (name, description) = if let Some(config_path) =
        locate_within(skill_dir, &skill_dir.join("config.yaml"))?
    {
        let cfg_text = read_within(skill_dir, &config_path)?;
        let cfg: SkillConfigHeader = serde_yaml::from_str(&cfg_text)?;
        (
            cfg.name.unwrap_or_else(|| skill.to_owned()),
            cfg.description.unwrap_or_default(),
        )
    } else {
        (skill.to_owned(), String::new())
    };

    let body = if let Some(index_md) = index_md {
        strip_frontmatter(&read_within(skill_dir, &index_md)?)
    } else if let Some(skill_md_path) = skill_md_path {
        strip_frontmatter(&read_within(skill_dir, &skill_md_path)?)
    } else {
        format!(
            "$(systemprompt core skills show {} --raw 2>/dev/null || echo \"Skill not available\")",
            skill
        )
    };

    Ok(format!(
        "---\nname: \"{}\"\ndescription: \"{}\"\n---\n\n{}\n",
        name,
        description.replace('"', "\\\""),
        body.trim()
    ))
}
