//! Codex CLI sync emitter.
//!
//! Writes manifest-supplied skills and MCP servers as one Codex plugin bundle
//! under `~/.codex/plugins/cache/<marketplace>/<plugin>/<version>/`, and
//! toggles its `[plugins."<plugin>@<marketplace>"]` block in `config.toml`
//! while preserving every other key.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::Serialize;

use crate::gateway::manifest::{ManagedMcpServer, SignedManifest, SkillEntry};
use crate::sync::host_sync::{HostSync, HostSyncCtx};
use crate::sync::{ApplyError, TomlError, safe_id_segment};

use super::config::{codex_home, user_config_path};
use super::probe::write_dotted;

const MARKETPLACE: &str = "systemprompt";
const PLUGIN_NAME: &str = "systemprompt-managed";
// The bundle is rewritten on each apply, so one fixed slot suffices; the real
// version travels in plugin.json.
const PLUGIN_VERSION_DIR: &str = "current";

#[derive(Clone, Copy, Debug)]
pub struct CodexCliSync;

#[async_trait]
impl HostSync for CodexCliSync {
    fn host_id(&self) -> &'static str {
        "codex-cli"
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        write_plugin_bundle(ctx.manifest)?;
        write_plugin_block(true)?;
        Ok(())
    }

    fn clear(&self) -> Result<(), ApplyError> {
        remove_plugin_bundle()?;
        write_plugin_block(false)?;
        Ok(())
    }
}

fn plugin_id() -> String {
    format!("{PLUGIN_NAME}@{MARKETPLACE}")
}

fn plugin_root() -> PathBuf {
    codex_home()
        .join("plugins")
        .join("cache")
        .join(MARKETPLACE)
        .join(PLUGIN_NAME)
        .join(PLUGIN_VERSION_DIR)
}

fn write_plugin_bundle(manifest: &SignedManifest) -> Result<(), ApplyError> {
    let root = plugin_root();
    if root.exists() {
        fs::remove_dir_all(&root).map_err(|e| ApplyError::Io {
            context: format!("clear {}", root.display()),
            source: e,
        })?;
    }
    let has_content = !manifest.skills.is_empty() || !manifest.managed_mcp_servers.is_empty();
    if !has_content {
        return Ok(());
    }
    fs::create_dir_all(&root).map_err(|e| ApplyError::Io {
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
    Ok(())
}

fn remove_plugin_bundle() -> Result<(), ApplyError> {
    let root = plugin_root();
    if root.exists() {
        fs::remove_dir_all(&root).map_err(|e| ApplyError::Io {
            context: format!("remove {}", root.display()),
            source: e,
        })?;
    }
    Ok(())
}

#[derive(Serialize)]
struct PluginJson<'a> {
    name: &'a str,
    version: &'a str,
    description: &'a str,
}

fn write_plugin_json(root: &Path, manifest: &SignedManifest) -> Result<(), ApplyError> {
    let dir = root.join(".codex-plugin");
    fs::create_dir_all(&dir).map_err(|e| ApplyError::Io {
        context: format!("create {}", dir.display()),
        source: e,
    })?;
    let pj = PluginJson {
        name: PLUGIN_NAME,
        version: manifest.manifest_version.as_str(),
        description: "Skills and MCP servers managed by your systemprompt.io organization.",
    };
    let bytes = serde_json::to_vec_pretty(&pj).map_err(|e| ApplyError::Serialize {
        what: "codex plugin.json".into(),
        source: e,
    })?;
    let path = dir.join("plugin.json");
    fs::write(&path, bytes).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
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
    headers: BTreeMap<&'a str, String>,
}

fn write_mcp_json(root: &Path, servers: &[ManagedMcpServer]) -> Result<(), ApplyError> {
    let bearer = crate::proxy::loopback_bearer().map_err(|e| ApplyError::Io {
        context: "read loopback secret for codex .mcp.json".into(),
        source: e,
    })?;
    let mcp_servers: BTreeMap<String, McpServerEntry<'_>> = servers
        .iter()
        .map(|s| {
            let slug = crate::mcp_registry::normalize_key(s.name.as_str());
            let url = crate::proxy::mcp_url(&slug);
            let mut headers = BTreeMap::new();
            headers.insert("Authorization", bearer.clone());
            (
                slug,
                McpServerEntry {
                    transport: s.transport.as_deref().unwrap_or("http"),
                    url,
                    headers,
                },
            )
        })
        .collect();
    let payload = McpJson { mcp_servers };
    let bytes = serde_json::to_vec_pretty(&payload).map_err(|e| ApplyError::Serialize {
        what: "codex .mcp.json".into(),
        source: e,
    })?;
    let path = root.join(".mcp.json");
    fs::write(&path, bytes).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

fn write_skill(root: &Path, skill: &SkillEntry) -> Result<(), ApplyError> {
    if !safe_id_segment(skill.id.as_str()) {
        return Err(ApplyError::UnsafeSkillId(skill.id.clone()));
    }
    let dir = root.join("skills").join(skill.id.as_str());
    fs::create_dir_all(&dir).map_err(|e| ApplyError::Io {
        context: format!("create {}", dir.display()),
        source: e,
    })?;
    let path = dir.join("SKILL.md");
    fs::write(&path, skill_markdown(skill)).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

fn skill_markdown(skill: &SkillEntry) -> String {
    let trimmed = skill.instructions.trim_start();
    if trimmed.starts_with("---") {
        return ensure_trailing_newline(skill.instructions.clone());
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
    ensure_trailing_newline(out)
}

fn ensure_trailing_newline(mut s: String) -> String {
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
    let escaped = s.replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn write_plugin_block(enabled: bool) -> Result<(), ApplyError> {
    let path = user_config_path();
    let mut value = read_or_empty_toml(&path)?;
    strip_managed_plugin_block(&mut value);
    write_dotted(
        &mut value,
        &format!("plugins.\"{}\".enabled", plugin_id()),
        toml::Value::Boolean(enabled),
    );

    let rendered = toml::to_string_pretty(&value).map_err(|e| ApplyError::Toml {
        what: format!("serialize {}", path.display()),
        source: TomlError::from(e),
    })?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| ApplyError::Io {
            context: format!("create {}", parent.display()),
            source: e,
        })?;
    }
    fs::write(&path, rendered).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })?;
    Ok(())
}

fn read_or_empty_toml(path: &Path) -> Result<toml::Value, ApplyError> {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(ApplyError::Io {
                context: format!("read {}", path.display()),
                source: e,
            });
        },
    };
    if raw.is_empty() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    toml::from_str::<toml::Value>(&raw).map_err(|e| ApplyError::Toml {
        what: format!("parse {}", path.display()),
        source: TomlError::from(e),
    })
}

fn strip_managed_plugin_block(root: &mut toml::Value) {
    let toml::Value::Table(top) = root else {
        return;
    };
    let Some(toml::Value::Table(plugins)) = top.get_mut("plugins") else {
        return;
    };
    plugins.remove(&plugin_id());
    if plugins.is_empty() {
        top.remove("plugins");
    }
}
