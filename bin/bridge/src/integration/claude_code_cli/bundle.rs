//! Writes the on-disk plugin bundle the Claude CLI loads: the
//! `.claude-plugin/plugin.json` descriptor, an optional `.mcp.json`, and one
//! Markdown file per managed skill and agent.

use std::fs;
use std::path::Path;

use serde_json::{Map, json};

use super::json_io::write_json;
use super::{PLUGIN_NAME, io_err};
use crate::gateway::manifest::{AgentEntry, ManagedMcpServer, SignedManifest, SkillEntry};
use crate::sync::{ApplyError, safe_id_segment};

pub(super) fn write_bundle(root: &Path, manifest: &SignedManifest) -> Result<(), ApplyError> {
    if root.exists() {
        fs::remove_dir_all(root).map_err(|e| io_err(format!("clear {}", root.display()), e))?;
    }
    fs::create_dir_all(root).map_err(|e| io_err(format!("create {}", root.display()), e))?;

    write_plugin_json(root, manifest)?;
    if !manifest.managed_mcp_servers.is_empty() {
        write_mcp_json(root, &manifest.managed_mcp_servers)?;
    }
    for skill in &manifest.skills {
        write_skill(root, skill)?;
    }
    for agent in &manifest.agents {
        write_agent(root, agent)?;
    }
    Ok(())
}

pub(super) fn remove_dir(path: &Path) -> Result<(), ApplyError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_err(format!("remove {}", path.display()), e)),
    }
}

fn write_plugin_json(root: &Path, manifest: &SignedManifest) -> Result<(), ApplyError> {
    let dir = root.join(".claude-plugin");
    fs::create_dir_all(&dir).map_err(|e| io_err(format!("create {}", dir.display()), e))?;
    let value = json!({
        "name": PLUGIN_NAME,
        "version": manifest.manifest_version.as_str(),
        "description": "Skills, agents, and MCP servers managed by your systemprompt.io organization.",
    });
    write_json(&dir.join("plugin.json"), &value)
}

fn write_mcp_json(root: &Path, servers: &[ManagedMcpServer]) -> Result<(), ApplyError> {
    let bearer = crate::proxy::loopback_bearer()
        .map_err(|e| io_err("read loopback secret for claude-code .mcp.json", e))?;
    let mut map = Map::new();
    for s in servers {
        let slug = crate::mcp_registry::normalize_key(s.name.as_str());
        map.insert(
            slug.clone(),
            json!({
                "type": s.transport.as_deref().unwrap_or("http"),
                "url": crate::proxy::mcp_url(&slug),
                "headers": { "Authorization": bearer.clone() },
            }),
        );
    }
    write_json(&root.join(".mcp.json"), &json!({ "mcpServers": map }))
}

fn write_skill(root: &Path, skill: &SkillEntry) -> Result<(), ApplyError> {
    if !safe_id_segment(skill.id.as_str()) {
        return Err(ApplyError::UnsafeSkillId(skill.id.clone()));
    }
    let dir = root.join("skills").join(skill.id.as_str());
    fs::create_dir_all(&dir).map_err(|e| io_err(format!("create {}", dir.display()), e))?;
    fs::write(dir.join("SKILL.md"), skill_markdown(skill))
        .map_err(|e| io_err(format!("write SKILL.md in {}", dir.display()), e))
}

fn write_agent(root: &Path, agent: &AgentEntry) -> Result<(), ApplyError> {
    if !safe_id_segment(agent.name.as_str()) {
        return Err(ApplyError::UnsafeAgentName(agent.name.to_string()));
    }
    let dir = root.join("agents");
    fs::create_dir_all(&dir).map_err(|e| io_err(format!("create {}", dir.display()), e))?;
    fs::write(
        dir.join(format!("{}.md", agent.name)),
        agent_markdown(agent),
    )
    .map_err(|e| io_err(format!("write agent in {}", dir.display()), e))
}

fn skill_markdown(skill: &SkillEntry) -> String {
    if skill.instructions.trim_start().starts_with("---") {
        return ensure_newline(skill.instructions.clone());
    }
    let mut out = String::from("---\n");
    out.push_str(&format!("name: {}\n", skill.name.as_str()));
    out.push_str(&format!(
        "description: {}\n",
        yaml_scalar(&skill.description)
    ));
    out.push_str("---\n\n");
    out.push_str(&skill.instructions);
    ensure_newline(out)
}

fn agent_markdown(agent: &AgentEntry) -> String {
    let mut out = String::from("---\n");
    out.push_str(&format!("name: {}\n", agent.name.as_str()));
    out.push_str(&format!(
        "description: {}\n",
        yaml_scalar(&agent.description)
    ));
    if let Some(model) = &agent.model {
        out.push_str(&format!("model: {}\n", yaml_scalar(model)));
    }
    out.push_str("---\n\n");
    match &agent.system_prompt {
        Some(p) => out.push_str(p),
        None => out.push_str(&format!(
            "# {}\n\n{}",
            agent.display_name, agent.description
        )),
    }
    ensure_newline(out)
}

fn ensure_newline(mut s: String) -> String {
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

fn yaml_scalar(s: &str) -> String {
    let needs_quotes = s.contains(':')
        || s.contains('#')
        || s.starts_with(['-', '?', '!', '&', '*', '|', '>', '\'', '"', '%', '@', '`']);
    if !needs_quotes {
        return s.to_owned();
    }
    format!("\"{}\"", s.replace('"', "\\\""))
}
