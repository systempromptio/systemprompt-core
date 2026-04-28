use super::super::hash::safe_id_segment;
use crate::manifest::AgentEntry;
use crate::paths;
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
            id: &a.id,
            name: &a.name,
            display_name: &a.display_name,
            version: &a.version,
            endpoint: &a.endpoint,
            is_default: a.is_default,
            is_primary: a.is_primary,
        }
    }
}

pub fn write_agents(meta_dir: &Path, agents: &[AgentEntry]) -> Result<(), String> {
    let dir = meta_dir.join(paths::AGENTS_DIR);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("clear agents dir: {e}"))?;
    }
    fs::create_dir_all(&dir).map_err(|e| format!("create agents dir: {e}"))?;

    write_index(&dir, agents)?;
    for agent in agents {
        write_one_agent(&dir, agent)?;
    }
    Ok(())
}

fn write_index(dir: &Path, agents: &[AgentEntry]) -> Result<(), String> {
    let index: Vec<AgentIndexEntry<'_>> = agents.iter().map(AgentIndexEntry::from).collect();
    let bytes =
        serde_json::to_vec_pretty(&index).map_err(|e| format!("serialize agents index: {e}"))?;
    fs::write(dir.join("index.json"), bytes).map_err(|e| format!("write agents index: {e}"))
}

fn write_one_agent(dir: &Path, agent: &AgentEntry) -> Result<(), String> {
    if !safe_id_segment(&agent.name) {
        return Err(format!(
            "manifest contained unsafe agent name: {}",
            agent.name
        ));
    }
    let path = dir.join(format!("{}.json", agent.name));
    let bytes = serde_json::to_vec_pretty(agent)
        .map_err(|e| format!("serialize agent {}: {e}", agent.name))?;
    fs::write(&path, bytes).map_err(|e| format!("write {}: {e}", path.display()))
}
