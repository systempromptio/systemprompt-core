//! Codex CLI sync emitter.
//!
//! Codex only loads plugins it resolves *through a marketplace*; a bare plugin
//! folder plus a `[plugins.*].enabled` flag is ignored. So skills ship as a
//! bridge-owned local marketplace (`marketplace.json` + `plugins/<name>/…`)
//! registered in `config.toml`, and Codex installs it into its own
//! `plugins/cache/` on launch — which is why we never write that cache.
//!
//! MCP rides a top-level `[mcp_servers.<slug>]` instead of the plugin bundle so
//! the connector survives even if the plugin/skills path fails. The source tree
//! is content-hashed and left byte-stable when unchanged, so Codex never sees a
//! spurious source change and re-installs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::Serialize;

use crate::gateway::manifest::{ManagedMcpServer, SignedManifest, SkillEntry};
use crate::sync::host_sync::{HostSync, HostSyncCtx};
use crate::sync::{ApplyError, TomlError, safe_id_segment, sha256_hex};

use super::config::{codex_home, user_config_path};
use super::probe::write_dotted;

const MARKETPLACE: &str = "systemprompt";
const PLUGIN_NAME: &str = "systemprompt-managed";

#[derive(Clone, Copy, Debug)]
pub struct CodexCliSync;

#[async_trait]
impl HostSync for CodexCliSync {
    fn host_id(&self) -> &'static str {
        "codex-cli"
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        let has_content =
            !ctx.manifest.skills.is_empty() || !ctx.manifest.managed_mcp_servers.is_empty();
        if has_content {
            write_marketplace_tree(ctx.manifest)?;
            write_config_blocks(true, &ctx.manifest.managed_mcp_servers)?;
        } else {
            remove_marketplace_tree()?;
            write_config_blocks(false, &[])?;
        }
        Ok(())
    }

    fn clear(&self) -> Result<(), ApplyError> {
        remove_marketplace_tree()?;
        write_config_blocks(false, &[])?;
        Ok(())
    }
}

fn plugin_id() -> String {
    format!("{PLUGIN_NAME}@{MARKETPLACE}")
}

fn marketplace_root() -> PathBuf {
    codex_home().join(".systemprompt").join("marketplace")
}

fn plugin_src_dir() -> PathBuf {
    marketplace_root().join("plugins").join(PLUGIN_NAME)
}

fn cache_plugin_dir() -> PathBuf {
    codex_home()
        .join("plugins")
        .join("cache")
        .join(MARKETPLACE)
        .join(PLUGIN_NAME)
}

fn write_marketplace_tree(manifest: &SignedManifest) -> Result<(), ApplyError> {
    let root = marketplace_root();
    let plugin_dir = plugin_src_dir();
    let version = bundle_version(manifest);

    let source_current = read_existing_version(&plugin_dir).as_deref() == Some(version.as_str())
        && root.join(".agents/plugins/marketplace.json").is_file();
    if !source_current {
        if plugin_dir.exists() {
            fs::remove_dir_all(&plugin_dir)
                .map_err(|e| io_err("clear plugin source", &plugin_dir, e))?;
        }
        fs::create_dir_all(&plugin_dir)
            .map_err(|e| io_err("create plugin source", &plugin_dir, e))?;
        write_marketplace_json(&root)?;
        write_plugin_json(&plugin_dir, &version)?;
        for skill in &manifest.skills {
            write_skill(&plugin_dir, skill)?;
        }
    }

    install_into_cache(&plugin_dir, &version)
}

// Codex marks a plugin "installed" solely by the presence of a version dir
// under its managed cache, so mirror what `codex plugin add` does (a copy into
// `cache/<marketplace>/<plugin>/<version>/`) rather than shelling out to it.
fn install_into_cache(plugin_dir: &Path, version: &str) -> Result<(), ApplyError> {
    let base = cache_plugin_dir();
    if let Ok(entries) = fs::read_dir(&base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy() == version || !path.is_dir() {
                continue;
            }
            if let Err(e) = fs::remove_dir_all(&path) {
                tracing::debug!(error = %e, path = %path.display(), "leaving stale codex plugin cache dir");
            }
        }
    }

    let dst = base.join(version);
    if read_existing_version(&dst).as_deref() == Some(version) {
        return Ok(());
    }
    if dst.exists() {
        fs::remove_dir_all(&dst).map_err(|e| io_err("clear cache install", &dst, e))?;
    }
    copy_dir_all(plugin_dir, &dst)
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), ApplyError> {
    fs::create_dir_all(dst).map_err(|e| io_err("create", dst, e))?;
    for entry in fs::read_dir(src).map_err(|e| io_err("read dir", src, e))? {
        let entry = entry.map_err(|e| io_err("read entry", src, e))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| io_err("copy", &from, e))?;
        }
    }
    Ok(())
}

fn remove_marketplace_tree() -> Result<(), ApplyError> {
    for dir in [marketplace_root(), cache_plugin_dir()] {
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| io_err("remove", &dir, e))?;
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct MarketplaceJson<'a> {
    name: &'a str,
    interface: MarketInterface<'a>,
    plugins: Vec<MarketPlugin<'a>>,
}

#[derive(Serialize)]
struct MarketInterface<'a> {
    #[serde(rename = "displayName")]
    display_name: &'a str,
}

#[derive(Serialize)]
struct MarketPlugin<'a> {
    name: &'a str,
    source: MarketSource<'a>,
    policy: MarketPolicy<'a>,
    category: &'a str,
}

#[derive(Serialize)]
struct MarketSource<'a> {
    source: &'a str,
    path: &'a str,
}

#[derive(Serialize)]
struct MarketPolicy<'a> {
    installation: &'a str,
    authentication: &'a str,
}

fn write_marketplace_json(root: &Path) -> Result<(), ApplyError> {
    let dir = root.join(".agents").join("plugins");
    fs::create_dir_all(&dir).map_err(|e| io_err("create marketplace dir", &dir, e))?;
    let plugin_rel = format!("./plugins/{PLUGIN_NAME}");
    let value = MarketplaceJson {
        name: MARKETPLACE,
        interface: MarketInterface {
            display_name: "Systemprompt managed",
        },
        plugins: vec![MarketPlugin {
            name: PLUGIN_NAME,
            source: MarketSource {
                source: "local",
                path: &plugin_rel,
            },
            policy: MarketPolicy {
                installation: "INSTALLED_BY_DEFAULT",
                authentication: "ON_INSTALL",
            },
            category: "Productivity",
        }],
    };
    let bytes = serde_json::to_vec_pretty(&value).map_err(|e| ApplyError::Serialize {
        what: "codex marketplace.json".into(),
        source: e,
    })?;
    let path = dir.join("marketplace.json");
    fs::write(&path, bytes).map_err(|e| io_err("write marketplace.json", &path, e))
}

#[derive(Serialize)]
struct PluginJson<'a> {
    name: &'a str,
    version: &'a str,
    description: &'a str,
    skills: &'a str,
    interface: PluginInterface<'a>,
}

#[derive(Serialize)]
struct PluginInterface<'a> {
    #[serde(rename = "displayName")]
    display_name: &'a str,
}

fn write_plugin_json(plugin_dir: &Path, version: &str) -> Result<(), ApplyError> {
    let dir = plugin_dir.join(".codex-plugin");
    fs::create_dir_all(&dir).map_err(|e| io_err("create .codex-plugin", &dir, e))?;
    let value = PluginJson {
        name: PLUGIN_NAME,
        version,
        description: "Skills managed by your systemprompt.io organization.",
        skills: "./skills/",
        interface: PluginInterface {
            display_name: "Systemprompt managed",
        },
    };
    let bytes = serde_json::to_vec_pretty(&value).map_err(|e| ApplyError::Serialize {
        what: "codex plugin.json".into(),
        source: e,
    })?;
    let path = dir.join("plugin.json");
    fs::write(&path, bytes).map_err(|e| io_err("write plugin.json", &path, e))
}

fn read_existing_version(plugin_dir: &Path) -> Option<String> {
    let bytes = fs::read(plugin_dir.join(".codex-plugin").join("plugin.json")).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value.get("version")?.as_str().map(str::to_owned)
}

// Hashes delivered content, not the gateway's per-request manifest_version,
// so the source tree stays byte-stable across polls when nothing changed.
fn bundle_version(manifest: &SignedManifest) -> String {
    let mut skills: Vec<&SkillEntry> = manifest.skills.iter().collect();
    skills.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

    let mut buf = String::new();
    for s in skills {
        buf.push_str(s.id.as_str());
        buf.push('\u{0}');
        buf.push_str(&skill_markdown(s));
        buf.push('\u{0}');
    }
    buf.push('\u{1}');

    let mut servers: Vec<(String, String)> = manifest
        .managed_mcp_servers
        .iter()
        .map(|s| {
            let slug = crate::mcp_registry::normalize_key(s.name.as_str());
            let url = crate::proxy::mcp_url(&slug);
            (slug, url)
        })
        .collect();
    servers.sort();
    for (slug, url) in servers {
        buf.push_str(&slug);
        buf.push('\u{0}');
        buf.push_str(&url);
        buf.push('\u{0}');
    }

    sha256_hex(buf.as_bytes())[..16].to_owned()
}

fn write_skill(plugin_dir: &Path, skill: &SkillEntry) -> Result<(), ApplyError> {
    if !safe_id_segment(skill.id.as_str()) {
        return Err(ApplyError::UnsafeSkillId(skill.id.clone()));
    }
    let dir = plugin_dir.join("skills").join(skill.id.as_str());
    fs::create_dir_all(&dir).map_err(|e| io_err("create skill dir", &dir, e))?;
    let path = dir.join("SKILL.md");
    fs::write(&path, skill_markdown(skill)).map_err(|e| io_err("write SKILL.md", &path, e))
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

fn write_config_blocks(enabled: bool, mcp_servers: &[ManagedMcpServer]) -> Result<(), ApplyError> {
    let path = user_config_path();
    let mut value = read_or_empty_toml(&path)?;
    let original = value.clone();

    if enabled {
        let root = marketplace_root();
        // Merge rather than replace the block: Codex stamps `last_updated` here,
        // and dropping it forces a needless re-sync.
        write_dotted(
            &mut value,
            &format!("marketplaces.{MARKETPLACE}.source_type"),
            toml::Value::String("local".to_owned()),
        );
        write_dotted(
            &mut value,
            &format!("marketplaces.{MARKETPLACE}.source"),
            toml::Value::String(root.display().to_string()),
        );
        write_dotted(
            &mut value,
            &format!("plugins.\"{}\".enabled", plugin_id()),
            toml::Value::Boolean(true),
        );
    } else {
        remove_marketplace_registration(&mut value);
    }

    // The loopback URL identifies our entries, letting us drop/rewrite them with
    // no persistent state while leaving foreign servers (e.g. `node_repl`) intact.
    strip_bridge_mcp_servers(&mut value);
    if enabled {
        write_mcp_servers(&mut value, mcp_servers)?;
    }

    if value == original {
        return Ok(());
    }

    let rendered = toml::to_string_pretty(&value).map_err(|e| ApplyError::Toml {
        what: format!("serialize {}", path.display()),
        source: TomlError::from(e),
    })?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io_err("create config dir", parent, e))?;
    }
    fs::write(&path, rendered).map_err(|e| io_err("write config.toml", &path, e))
}

fn write_mcp_servers(
    value: &mut toml::Value,
    servers: &[ManagedMcpServer],
) -> Result<(), ApplyError> {
    if servers.is_empty() {
        return Ok(());
    }
    let bearer = crate::proxy::loopback_bearer().map_err(|e| ApplyError::Io {
        context: "read loopback secret for codex mcp_servers".into(),
        source: e,
    })?;
    for s in servers {
        let slug = crate::mcp_registry::normalize_key(s.name.as_str());
        write_dotted(
            value,
            &format!("mcp_servers.{slug}.url"),
            toml::Value::String(crate::proxy::mcp_url(&slug)),
        );
        write_dotted(
            value,
            &format!("mcp_servers.{slug}.http_headers.Authorization"),
            toml::Value::String(bearer.clone()),
        );
    }
    Ok(())
}

fn read_or_empty_toml(path: &Path) -> Result<toml::Value, ApplyError> {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(io_err("read config.toml", path, e)),
    };
    if raw.is_empty() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    toml::from_str::<toml::Value>(&raw).map_err(|e| ApplyError::Toml {
        what: format!("parse {}", path.display()),
        source: TomlError::from(e),
    })
}

fn remove_marketplace_registration(root: &mut toml::Value) {
    let Some(top) = root.as_table_mut() else {
        return;
    };
    if let Some(toml::Value::Table(plugins)) = top.get_mut("plugins") {
        plugins.remove(&plugin_id());
        if plugins.is_empty() {
            top.remove("plugins");
        }
    }
    if let Some(toml::Value::Table(markets)) = top.get_mut("marketplaces") {
        markets.remove(MARKETPLACE);
        if markets.is_empty() {
            top.remove("marketplaces");
        }
    }
}

fn strip_bridge_mcp_servers(root: &mut toml::Value) {
    let Some(top) = root.as_table_mut() else {
        return;
    };
    let Some(toml::Value::Table(servers)) = top.get_mut("mcp_servers") else {
        return;
    };
    let prefix = format!("{}/mcp/", crate::proxy::loopback_origin());
    servers.retain(|_name, entry| {
        let is_ours = entry
            .get("url")
            .and_then(toml::Value::as_str)
            .is_some_and(|u| u.starts_with(&prefix));
        !is_ours
    });
    if servers.is_empty() {
        top.remove("mcp_servers");
    }
}

fn io_err(context: &str, path: &Path, source: std::io::Error) -> ApplyError {
    ApplyError::Io {
        context: format!("{context} {}", path.display()),
        source,
    }
}
