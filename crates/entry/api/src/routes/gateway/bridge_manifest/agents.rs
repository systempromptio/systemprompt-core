//! Agent discovery for the bridge manifest.
//!
//! Projects enabled `AgentConfig` entries from the services config into
//! `AgentEntry` records, resolving endpoints against the API external URL.

use systemprompt_identifiers::{AgentId, AgentName};
use systemprompt_models::bridge::manifest::AgentEntry;
use systemprompt_models::services::{AgentConfig, ServicesConfig};

#[doc(hidden)]
pub fn load_agents(services: &ServicesConfig, api_external_url: &str) -> Vec<AgentEntry> {
    let base = api_external_url.trim_end_matches('/');
    let mut keys: Vec<&String> = services
        .agents
        .iter()
        .filter(|(_, cfg)| cfg.enabled)
        .map(|(k, _)| k)
        .collect();
    keys.sort();

    let mut out = Vec::with_capacity(keys.len());
    for key in keys {
        let cfg = &services.agents[key];
        match build_agent_entry(key, cfg, base) {
            Ok(entry) => out.push(entry),
            Err(e) => {
                tracing::warn!(
                    agent = %key,
                    error = %e,
                    "manifest: failed to build agent entry; skipping"
                );
            },
        }
    }
    out
}

fn build_agent_entry(key: &str, cfg: &AgentConfig, base: &str) -> anyhow::Result<AgentEntry> {
    let id = AgentId::new(key);
    let name = AgentName::try_new(cfg.name.clone())?;
    let endpoint = if cfg.endpoint.starts_with("http://") || cfg.endpoint.starts_with("https://") {
        cfg.endpoint.clone()
    } else if cfg.endpoint.is_empty() {
        format!("{base}/api/v1/agents/{}", cfg.name)
    } else {
        format!("{base}{}", cfg.endpoint)
    };

    let display_name = cfg.card.display_name.clone();
    let description = cfg.card.description.clone();
    let version = cfg.card.version.clone();

    Ok(AgentEntry {
        id,
        name,
        display_name,
        description,
        version,
        endpoint,
        enabled: cfg.enabled,
        is_default: cfg.default,
        is_primary: cfg.is_primary,
        provider: cfg.metadata.provider.clone(),
        model: cfg.metadata.model.clone(),
        mcp_servers: cfg.metadata.mcp_servers.clone(),
        skills: cfg.metadata.skills.clone(),
        tags: cfg.tags.clone(),
        system_prompt: cfg.metadata.system_prompt.clone(),
    })
}
