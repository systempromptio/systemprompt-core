//! Agent markdown generation for plugin bundles.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use serde::Deserialize;
use serde::de::IgnoredAny;
use std::collections::BTreeMap;
use std::path::Path;
use systemprompt_models::{ComponentSource, PluginConfig};

use super::DEFAULT_AGENT_TOOLS;

#[derive(Debug, Deserialize)]
struct AgentNames {
    #[serde(default)]
    agents: BTreeMap<String, IgnoredAny>,
}

#[derive(Debug, Deserialize)]
struct AgentsFile {
    #[serde(default)]
    agents: BTreeMap<String, AgentHeader>,
}

#[derive(Debug, Deserialize)]
struct AgentHeader {
    card: Option<AgentCardHeader>,
    metadata: Option<AgentMetadataHeader>,
}

#[derive(Debug, Deserialize)]
struct AgentCardHeader {
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AgentMetadataHeader {
    #[serde(rename = "systemPrompt")]
    system_prompt: Option<String>,
}

pub fn generate_agents(
    plugin: &PluginConfig,
    services_path: &Path,
    output_dir: &Path,
    files_generated: &mut Vec<String>,
) -> Result<()> {
    let agents_dir = output_dir.join("agents");

    let agents = resolve_agents(plugin, services_path)?;

    if agents.is_empty() {
        return Ok(());
    }

    std::fs::create_dir_all(&agents_dir)?;

    let services_agents_dir = services_path.join("agents");

    for agent in &agents {
        let agent_md = build_agent_md(agent, &services_agents_dir)?;
        let agent_path = agents_dir.join(format!("{agent}.md"));
        std::fs::write(&agent_path, &agent_md)?;
        files_generated.push(agent_path.to_string_lossy().to_string());
    }

    Ok(())
}

fn resolve_agents(plugin: &PluginConfig, services_path: &Path) -> Result<Vec<String>> {
    if plugin.agents.source == ComponentSource::Explicit {
        return Ok(plugin.agents.include.clone());
    }

    let agents_config_path = services_path.join("config").join("config.yaml");
    if !agents_config_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&agents_config_path)?;
    let config: AgentNames = serde_yaml::from_str(&content)?;

    Ok(config
        .agents
        .into_keys()
        .filter(|name| !plugin.agents.exclude.contains(name))
        .collect())
}

fn build_agent_md(agent: &str, services_agents_dir: &Path) -> Result<String> {
    if services_agents_dir.exists() {
        for entry in std::fs::read_dir(services_agents_dir)? {
            let entry = entry?;
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str());
            if ext != Some("yaml") && ext != Some("yml") {
                continue;
            }
            let content = std::fs::read_to_string(&path)?;
            let mut config: AgentsFile = match serde_yaml::from_str(&content) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "Failed to parse YAML");
                    continue;
                },
            };
            if let Some(header) = config.agents.remove(agent) {
                let description = header
                    .card
                    .and_then(|card| card.description)
                    .unwrap_or_else(|| format!("{agent} agent"));
                let system_prompt = header
                    .metadata
                    .and_then(|metadata| metadata.system_prompt)
                    .unwrap_or_default();
                return Ok(format!(
                    "---\nname: {}\ndescription: \"{}\"\ntools: {}\n---\n\n{}\n",
                    agent,
                    description.replace('"', "\\\""),
                    DEFAULT_AGENT_TOOLS,
                    system_prompt.trim()
                ));
            }
        }
    }

    Ok(format!(
        "---\nname: {}\ndescription: \"{} agent\"\ntools: {}\n---\n\nYou are the {} agent.\n",
        agent, agent, DEFAULT_AGENT_TOOLS, agent
    ))
}
