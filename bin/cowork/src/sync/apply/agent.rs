use super::super::hash::safe_id_segment;
use crate::config::paths;
use crate::gateway::manifest::AgentEntry;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
struct AgentIndexEntry<'a> {
    id: &'a str,
    name: &'a str,
    display_name: &'a str,
    version: &'a str,
    endpoint: &'a str,
    is_default: bool,
    is_primary: bool,
}

impl<'a> From<&'a AgentEntry> for AgentIndexEntry<'a> {
    fn from(a: &'a AgentEntry) -> Self {
        Self {
            id: a.id.as_str(),
            name: a.name.as_str(),
            display_name: &a.display_name,
            version: &a.version,
            endpoint: &a.endpoint,
            is_default: a.is_default,
            is_primary: a.is_primary,
        }
    }
}

#[tracing::instrument(level = "debug", skip(agents), fields(count = agents.len()))]
pub fn write_agents(meta_dir: &Path, agents: &[AgentEntry]) -> Result<(), super::ApplyError> {
    let dir = meta_dir.join(paths::AGENTS_DIR);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| super::ApplyError::Io {
            context: "clear agents dir".into(),
            source: e,
        })?;
    }
    fs::create_dir_all(&dir).map_err(|e| super::ApplyError::Io {
        context: "create agents dir".into(),
        source: e,
    })?;

    write_index(&dir, agents)?;
    for agent in agents {
        write_one_agent(&dir, agent)?;
    }
    Ok(())
}

fn write_index(dir: &Path, agents: &[AgentEntry]) -> Result<(), super::ApplyError> {
    let index: Vec<AgentIndexEntry<'_>> = agents.iter().map(AgentIndexEntry::from).collect();
    let bytes = serde_json::to_vec_pretty(&index).map_err(|e| super::ApplyError::Serialize {
        what: "agents index".into(),
        source: e,
    })?;
    fs::write(dir.join("index.json"), bytes).map_err(|e| super::ApplyError::Io {
        context: "write agents index".into(),
        source: e,
    })
}

fn write_one_agent(dir: &Path, agent: &AgentEntry) -> Result<(), super::ApplyError> {
    if !safe_id_segment(agent.name.as_str()) {
        return Err(super::ApplyError::UnsafeAgentName(agent.name.to_string()));
    }
    let path = dir.join(format!("{}.json", agent.name));
    let bytes = serde_json::to_vec_pretty(agent).map_err(|e| super::ApplyError::Serialize {
        what: format!("agent {}", agent.name),
        source: e,
    })?;
    fs::write(&path, bytes).map_err(|e| super::ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}
