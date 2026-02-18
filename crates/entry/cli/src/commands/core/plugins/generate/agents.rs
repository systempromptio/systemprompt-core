use anyhow::Result;
use std::path::Path;
use systemprompt_models::{ComponentSource, PluginConfig};

use super::DEFAULT_AGENT_TOOLS;

pub fn generate_agents(
    plugin: &PluginConfig,
    services_path: &Path,
    output_dir: &Path,
    files_generated: &mut Vec<String>,
) -> Result<()> {
    let agents_dir = output_dir.join("agents");

    let agent_ids = resolve_agent_ids(plugin, services_path)?;

    if agent_ids.is_empty() {
        return Ok(());
    }

    std::fs::create_dir_all(&agents_dir)?;

    let services_agents_dir = services_path.join("agents");

    for agent_id in &agent_ids {
        let agent_md = build_agent_md(agent_id, &services_agents_dir)?;
        let agent_path = agents_dir.join(format!("{agent_id}.md"));
        std::fs::write(&agent_path, &agent_md)?;
        files_generated.push(agent_path.to_string_lossy().to_string());
    }

    Ok(())
}

fn resolve_agent_ids(plugin: &PluginConfig, services_path: &Path) -> Result<Vec<String>> {
    if plugin.agents.source == ComponentSource::Explicit {
        return Ok(plugin.agents.include.clone());
    }

    let agents_config_path = services_path.join("config").join("config.yaml");
    if !agents_config_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&agents_config_path)?;
    let config: serde_yaml::Value = serde_yaml::from_str(&content)?;

    let mut ids = Vec::new();
    if let Some(agents) = config.get("agents").and_then(|a| a.as_mapping()) {
        for (key, _) in agents {
            if let Some(name) = key.as_str() {
                if !plugin.agents.exclude.contains(&name.to_string()) {
                    ids.push(name.to_string());
                }
            }
        }
    }

    ids.sort();
    Ok(ids)
}

fn build_agent_md(agent_id: &str, services_agents_dir: &Path) -> Result<String> {
    if services_agents_dir.exists() {
        for entry in std::fs::read_dir(services_agents_dir)? {
            let entry = entry?;
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str());
            if ext != Some("yaml") && ext != Some("yml") {
                continue;
            }
            let content = std::fs::read_to_string(&path)?;
            let config: serde_yaml::Value = match serde_yaml::from_str(&content) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "Failed to parse YAML");
                    continue;
                },
            };
            if let Some(agent_val) = config.get("agents").and_then(|a| a.get(agent_id)) {
                let description = agent_val
                    .get("card")
                    .and_then(|c| c.get("description"))
                    .and_then(|d| d.as_str())
                    .map_or_else(|| format!("{agent_id} agent"), ToString::to_string);
                let system_prompt = agent_val
                    .get("metadata")
                    .and_then(|m| m.get("systemPrompt"))
                    .and_then(|s| s.as_str())
                    .map_or_else(String::new, ToString::to_string);
                return Ok(format!(
                    "---\nname: {}\ndescription: \"{}\"\ntools: {}\n---\n\n{}\n",
                    agent_id,
                    description.replace('"', "\\\""),
                    DEFAULT_AGENT_TOOLS,
                    system_prompt.trim()
                ));
            }
        }
    }

    Ok(format!(
        "---\nname: {}\ndescription: \"{} agent\"\ntools: {}\n---\n\nYou are the {} agent.\n",
        agent_id, agent_id, DEFAULT_AGENT_TOOLS, agent_id
    ))
}
