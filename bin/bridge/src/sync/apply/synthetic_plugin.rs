use super::super::hash::safe_id_segment;
use super::hooks::{ensure_plugin_json_hooks_field, materialize_hook_token, write_hooks_json};
use crate::config::paths;
use crate::gateway::GatewayClient;
use crate::gateway::manifest::{AgentEntry, ManagedMcpServer, SignedManifest, SkillEntry};
use crate::sync::host_sync::{HostSync, HostSyncCtx};
use async_trait::async_trait;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) struct ClaudeCodePluginSync;

#[async_trait]
impl HostSync for ClaudeCodePluginSync {
    fn host_id(&self) -> &'static str {
        "claude-code"
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), super::ApplyError> {
        write_synthetic_plugin(ctx.client, ctx.bearer, ctx.org_plugins_root, ctx.manifest).await
    }

    fn clear(&self) -> Result<(), super::ApplyError> {
        let Some(location) = paths::org_plugins_effective() else {
            return Ok(());
        };
        let root = location.path.join(paths::SYNTHETIC_PLUGIN_NAME);
        if root.exists() {
            fs::remove_dir_all(&root).map_err(|e| super::ApplyError::Io {
                context: format!("remove {}", root.display()),
                source: e,
            })?;
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct PluginJson<'a> {
    name: &'a str,
    description: &'a str,
    version: &'a str,
    // Why: under MDM + custom inference gateway, the default `"available"`
    // surfaces Cowork's "Contact an organization owner to install connectors"
    // tooltip — the user cannot install. `"auto_install"` auto-installs the
    // plugin on first session-load while still allowing user-initiated uninstall.
    // Docs: https://claude.com/docs/cowork/3p/extensions
    #[serde(rename = "installationPreference")]
    installation_preference: &'a str,
}

pub const PLUGIN_INSTALLATION_PREFERENCE: &str = "auto_install";

// Pure JSON renderer for the synthetic plugin's `plugin.json`. Separated from
// the IO path so unit tests can pin the wire shape without needing a tempdir
// or a runtime.
pub fn render_plugin_json(manifest_version: &str) -> Vec<u8> {
    let pj = PluginJson {
        name: paths::SYNTHETIC_PLUGIN_NAME,
        description: "Skills, agents, and MCP servers managed by your organization.",
        version: manifest_version,
        installation_preference: PLUGIN_INSTALLATION_PREFERENCE,
    };
    serde_json::to_vec_pretty(&pj).expect("PluginJson serialization is infallible")
}

#[derive(Serialize)]
struct McpJson<'a> {
    #[serde(rename = "mcpServers")]
    mcp_servers: BTreeMap<String, McpServerEntry<'a>>,
}

#[derive(Serialize)]
struct McpServerEntry<'a> {
    #[serde(rename = "type")]
    transport: &'a str,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<BTreeMap<&'a str, String>>,
}

#[tracing::instrument(level = "debug", skip(client, bearer, manifest))]
pub async fn write_synthetic_plugin(
    client: &GatewayClient,
    bearer: &str,
    org_plugins_root: &Path,
    manifest: &SignedManifest,
) -> Result<(), super::ApplyError> {
    let root = org_plugins_root.join(paths::SYNTHETIC_PLUGIN_NAME);

    let has_content = !manifest.skills.is_empty()
        || !manifest.agents.is_empty()
        || !manifest.managed_mcp_servers.is_empty()
        || !manifest.hooks.is_empty();

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

    let synthetic_id = systemprompt_identifiers::PluginId::new(paths::SYNTHETIC_PLUGIN_NAME);
    materialize_hook_token(client, bearer, &synthetic_id, &root).await?;
    write_hooks_json(client.base_url_str(), &synthetic_id, &root, &manifest.hooks)?;
    ensure_plugin_json_hooks_field(&root)?;

    Ok(())
}

fn write_plugin_json(root: &Path, manifest: &SignedManifest) -> Result<(), super::ApplyError> {
    let dir = root.join(".claude-plugin");
    fs::create_dir_all(&dir).map_err(|e| super::ApplyError::Io {
        context: format!("create {}", dir.display()),
        source: e,
    })?;
    let bytes = render_plugin_json(manifest.manifest_version.as_str());
    let path = dir.join("plugin.json");
    fs::write(&path, bytes).map_err(|e| super::ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

fn write_mcp_json(root: &Path, servers: &[ManagedMcpServer]) -> Result<(), super::ApplyError> {
    let slugs: Vec<String> = servers
        .iter()
        .map(|s| crate::mcp_registry::normalize_key(s.name.as_str()))
        .collect();
    let mcp_servers: BTreeMap<String, McpServerEntry<'_>> = servers
        .iter()
        .zip(slugs.iter())
        .map(|(s, slug)| {
            let headers = s
                .headers
                .as_ref()
                .map(|m| m.iter().map(|(k, v)| (k.as_str(), v.clone())).collect());
            (
                slug.clone(),
                McpServerEntry {
                    transport: s.transport.as_deref().unwrap_or("http"),
                    url: s.url.as_str().to_string(),
                    headers,
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
