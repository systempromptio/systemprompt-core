//! Agent component projection: selecting a plugin's agents from the resolved
//! catalogue and laying them out as `agents/<id>.md`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::AgentId;
use systemprompt_models::bridge::manifest::AgentEntry;
use systemprompt_models::services::{ComponentSource, PluginConfig};

use super::{BundleFile, PluginBundle};

pub(super) fn resolve_agents(config: &PluginConfig, agents: &[AgentEntry]) -> Vec<AgentId> {
    match config.agents.source {
        ComponentSource::Explicit => config
            .agents
            .include
            .iter()
            .cloned()
            .map(AgentId::new)
            .collect(),
        ComponentSource::Instance => agents
            .iter()
            .map(|a| a.id.clone())
            .filter(|id| !config.agents.exclude.iter().any(|ex| ex == id.as_str()))
            .collect(),
    }
}

pub(super) fn append_agent_files(
    agents: &[AgentEntry],
    agent_ids: &[AgentId],
    bundle: &mut PluginBundle,
) {
    for agent in agents.iter().filter(|a| agent_ids.contains(&a.id)) {
        bundle.insert(
            format!("agents/{}.md", agent.id.as_str()),
            BundleFile {
                bytes: agent_md(agent).into_bytes(),
                executable: false,
            },
        );
    }
}

fn agent_md(agent: &AgentEntry) -> String {
    let body = match agent.system_prompt.as_deref().map(str::trim) {
        Some(prompt) if !prompt.is_empty() => prompt.to_owned(),
        _ => format!("# {}\n\n{}", agent.display_name, agent.description),
    };
    let mut front = format!(
        "---\nname: {}\ndescription: \"{}\"\n",
        agent.id.as_str(),
        agent.description.replace('"', "\\\"")
    );
    if let Some(model) = agent.model.as_deref().filter(|m| !m.is_empty()) {
        front.push_str(&format!("model: \"{}\"\n", model.replace('"', "\\\"")));
    }
    format!("{front}---\n\n{body}\n")
}
