use super::super::hash::safe_id_segment;
use crate::config::paths;
use crate::gateway::manifest::{AgentEntry, ManagedMcpServer, SignedManifest, SkillEntry};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct PluginJson<'a> {
    name: &'a str,
    description: &'a str,
    version: &'a str,
}

#[derive(Serialize)]
struct McpJson<'a> {
    #[serde(rename = "mcpServers")]
    mcp_servers: BTreeMap<&'a str, McpServerEntry<'a>>,
}

#[derive(Serialize)]
struct McpServerEntry<'a> {
    #[serde(rename = "type")]
    transport: &'a str,
    url: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<&'a std::collections::BTreeMap<String, String>>,
}

#[tracing::instrument(level = "debug", skip(manifest))]
pub fn write_synthetic_plugin(
    org_plugins_root: &Path,
    manifest: &SignedManifest,
) -> Result<(), super::ApplyError> {
    let root = org_plugins_root.join(paths::SYNTHETIC_PLUGIN_NAME);

    let has_content = !manifest.skills.is_empty()
        || !manifest.agents.is_empty()
        || !manifest.managed_mcp_servers.is_empty();

    if !has_content {
        if root.exists() {
            fs::remove_dir_all(&root).map_err(|e| super::ApplyError::Io {
                context: format!("remove {}", root.display()),
                source: e,
            })?;
        }
        return Ok(());
    }

    if root.exists() {
        fs::remove_dir_all(&root).map_err(|e| super::ApplyError::Io {
            context: format!("clear {}", root.display()),
            source: e,
        })?;
    }
    fs::create_dir_all(&root).map_err(|e| super::ApplyError::Io {
        context: format!("create {}", root.display()),
        source: e,
    })?;

    write_plugin_json(&root, manifest)?;

    if !manifest.managed_mcp_servers.is_empty() {
        write_mcp_json(&root, &manifest.managed_mcp_servers)?;
    }

    for skill in &manifest.skills {
        write_skill(&root, skill)?;
    }

    for agent in &manifest.agents {
        write_agent(&root, agent)?;
    }

    Ok(())
}

fn write_plugin_json(root: &Path, manifest: &SignedManifest) -> Result<(), super::ApplyError> {
    let dir = root.join(".claude-plugin");
    fs::create_dir_all(&dir).map_err(|e| super::ApplyError::Io {
        context: format!("create {}", dir.display()),
        source: e,
    })?;
    let pj = PluginJson {
        name: paths::SYNTHETIC_PLUGIN_NAME,
        description: "Skills, agents, and MCP servers managed by your organization.",
        version: manifest.manifest_version.as_str(),
    };
    let bytes = serde_json::to_vec_pretty(&pj).map_err(|e| super::ApplyError::Serialize {
        what: "synthetic plugin.json".into(),
        source: e,
    })?;
    let path = dir.join("plugin.json");
    fs::write(&path, bytes).map_err(|e| super::ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

fn write_mcp_json(root: &Path, servers: &[ManagedMcpServer]) -> Result<(), super::ApplyError> {
    let mcp_servers: BTreeMap<&str, McpServerEntry<'_>> = servers
        .iter()
        .map(|s| {
            (
                s.name.as_str(),
                McpServerEntry {
                    transport: s.transport.as_deref().unwrap_or("http"),
                    url: s.url.as_str(),
                    headers: s.headers.as_ref(),
                },
            )
        })
        .collect();
    let payload = McpJson { mcp_servers };
    let bytes = serde_json::to_vec_pretty(&payload).map_err(|e| super::ApplyError::Serialize {
        what: "synthetic .mcp.json".into(),
        source: e,
    })?;
    let path = root.join(".mcp.json");
    fs::write(&path, bytes).map_err(|e| super::ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

fn write_skill(root: &Path, skill: &SkillEntry) -> Result<(), super::ApplyError> {
    if !safe_id_segment(skill.id.as_str()) {
        return Err(super::ApplyError::UnsafeSkillId(skill.id.clone()));
    }
    let dir = root.join("skills").join(skill.id.as_str());
    fs::create_dir_all(&dir).map_err(|e| super::ApplyError::Io {
        context: format!("create {}", dir.display()),
        source: e,
    })?;
    let path = dir.join("SKILL.md");
    fs::write(&path, skill_markdown(skill)).map_err(|e| super::ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

fn skill_markdown(skill: &SkillEntry) -> String {
    let trimmed = skill.instructions.trim_start();
    if trimmed.starts_with("---") {
        return skill.instructions.clone();
    }
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", skill.name.as_str()));
    out.push_str(&format!(
        "description: {}\n",
        yaml_scalar(&skill.description)
    ));
    out.push_str("---\n\n");
    out.push_str(&skill.instructions);
    if !skill.instructions.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn yaml_scalar(s: &str) -> String {
    let needs_quotes = s.contains(':')
        || s.contains('#')
        || s.starts_with(['-', '?', '!', '&', '*', '|', '>', '\'', '"', '%', '@', '`']);
    if !needs_quotes {
        return s.to_string();
    }
    let escaped = s.replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn write_agent(root: &Path, agent: &AgentEntry) -> Result<(), super::ApplyError> {
    if !safe_id_segment(agent.name.as_str()) {
        return Err(super::ApplyError::UnsafeAgentName(agent.name.to_string()));
    }
    let dir = root.join("agents");
    fs::create_dir_all(&dir).map_err(|e| super::ApplyError::Io {
        context: format!("create {}", dir.display()),
        source: e,
    })?;
    let path: PathBuf = dir.join(format!("{}.md", agent.name));
    fs::write(&path, agent_markdown(agent)).map_err(|e| super::ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

fn agent_markdown(agent: &AgentEntry) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", agent.name.as_str()));
    out.push_str(&format!(
        "description: {}\n",
        yaml_scalar(&agent.description)
    ));
    if let Some(model) = &agent.model {
        out.push_str(&format!("model: {}\n", yaml_scalar(model)));
    }
    out.push_str("---\n\n");
    if let Some(prompt) = &agent.system_prompt {
        out.push_str(prompt);
        if !prompt.ends_with('\n') {
            out.push('\n');
        }
    } else {
        out.push_str(&format!(
            "# {}\n\n{}\n",
            agent.display_name, agent.description
        ));
    }
    out
}
