use anyhow::Result;
use std::path::{Path, PathBuf};
use systemprompt_models::{strip_frontmatter, ComponentFilter, ComponentSource, PluginConfig};

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
            let skill_dir = skills_path.join(skill_id);
            if skill_dir.exists() {
                resolved.push((skill_id.clone(), skill_dir));
            }
        }
        return Ok(resolved);
    }

    if !skills_path.exists() {
        return Ok(resolved);
    }

    for entry in std::fs::read_dir(skills_path)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let skill_id = entry.file_name().to_string_lossy().to_string();

        if plugin.skills.exclude.contains(&skill_id) {
            continue;
        }

        if plugin.skills.filter == Some(ComponentFilter::Enabled) {
            let config_path = path.join("config.yaml");
            if config_path.exists() {
                let cfg_text = std::fs::read_to_string(&config_path)?;
                let cfg: serde_yaml::Value = serde_yaml::from_str(&cfg_text)?;
                let enabled = cfg
                    .get("enabled")
                    .and_then(serde_yaml::Value::as_bool)
                    .unwrap_or(true);
                if !enabled {
                    continue;
                }
            }
        }

        resolved.push((skill_id, path));
    }

    resolved.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(resolved)
}

fn build_skill_md(skill_id: &str, skill_dir: &Path) -> Result<String> {
    let index_md = skill_dir.join("index.md");
    let skill_md_path = skill_dir.join("SKILL.md");

    let config_path = skill_dir.join("config.yaml");
    let (name, description) = if config_path.exists() {
        let cfg_text = std::fs::read_to_string(&config_path)?;
        let cfg: serde_yaml::Value = serde_yaml::from_str(&cfg_text)?;
        let name = cfg
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(skill_id)
            .to_string();
        let desc = cfg
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        (name, desc)
    } else {
        (skill_id.to_string(), String::new())
    };

    let body = if index_md.exists() {
        let content = std::fs::read_to_string(&index_md)?;
        strip_frontmatter(&content)
    } else if skill_md_path.exists() {
        let content = std::fs::read_to_string(&skill_md_path)?;
        strip_frontmatter(&content)
    } else {
        format!(
            "$(systemprompt core skills show {} --raw 2>/dev/null || echo \"Skill not available\")",
            skill_id
        )
    };

    Ok(format!(
        "---\nname: \"{}\"\ndescription: \"{}\"\n---\n\n{}\n",
        name,
        description.replace('"', "\\\""),
        body.trim()
    ))
}
