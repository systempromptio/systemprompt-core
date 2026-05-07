//! Codex CLI sync emitter.
//!
//! Writes manifest-supplied MCP servers and skills into Codex's native config
//! locations on every `apply_manifest` run, alongside the existing Cowork
//! synthetic-plugin and Windows MDM emitters.
//!
//! - MCP servers land as `[mcp_servers.sp_<slug>]` blocks in the host's managed
//!   config TOML, preserving any non-MCP keys (model_provider, otel, analytics)
//!   the install step previously wrote.
//! - Skills land as `~/.codex/skills/sp-<id>/SKILL.md` files.
//!
//! The bridge owns every entry whose key/dir starts with the `sp_` (TOML) /
//! `sp-` (skills) prefix and rewrites the full set on each sync; user-authored
//! entries without that prefix are left untouched.

use std::fs;
use std::path::Path;

use crate::gateway::manifest::{ManagedMcpServer, SkillEntry};
use crate::sync::host_sync::{HostSync, HostSyncCtx};
use crate::sync::{ApplyError, TomlError, safe_id_segment};

use super::config::{codex_home, managed_config_path};
use super::probe::write_dotted;

const MCP_KEY_PREFIX: &str = "sp_";
const SKILL_DIR_PREFIX: &str = "sp-";

pub struct CodexCliSync;

impl HostSync for CodexCliSync {
    fn host_id(&self) -> &'static str {
        "codex-cli"
    }

    fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        write_managed_mcp_servers(&ctx.manifest.managed_mcp_servers)?;
        write_managed_skills(&ctx.manifest.skills)?;
        Ok(())
    }

    fn clear(&self) -> Result<(), ApplyError> {
        write_managed_mcp_servers(&[])?;
        write_managed_skills(&[])?;
        Ok(())
    }
}

fn write_managed_mcp_servers(servers: &[ManagedMcpServer]) -> Result<(), ApplyError> {
    let path = managed_config_path();
    let mut value = read_or_empty_toml(&path)?;
    strip_managed_mcp_blocks(&mut value);

    for server in servers {
        let slug = format!(
            "{MCP_KEY_PREFIX}{}",
            crate::mcp_registry::normalize_key(server.name.as_str())
        );
        emit_server(&mut value, &slug, server);
    }

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

fn strip_managed_mcp_blocks(root: &mut toml::Value) {
    let toml::Value::Table(top) = root else {
        return;
    };
    let Some(toml::Value::Table(servers)) = top.get_mut("mcp_servers") else {
        return;
    };
    servers.retain(|k, _| !k.starts_with(MCP_KEY_PREFIX));
    if servers.is_empty() {
        top.remove("mcp_servers");
    }
}

fn emit_server(root: &mut toml::Value, slug: &str, server: &ManagedMcpServer) {
    let url_key = format!("mcp_servers.{slug}.url");
    write_dotted(
        root,
        &url_key,
        toml::Value::String(server.url.as_str().to_string()),
    );

    if let Some(headers) = &server.headers
        && !headers.is_empty()
    {
        let mut table = toml::map::Map::new();
        for (k, v) in headers {
            table.insert(k.clone(), toml::Value::String(v.clone()));
        }
        let key = format!("mcp_servers.{slug}.http_headers");
        write_dotted(root, &key, toml::Value::Table(table));
    }

    write_dotted(
        root,
        &format!("mcp_servers.{slug}.enabled"),
        toml::Value::Boolean(true),
    );
    write_dotted(
        root,
        &format!("mcp_servers.{slug}.startup_timeout_sec"),
        toml::Value::Integer(10),
    );
    write_dotted(
        root,
        &format!("mcp_servers.{slug}.tool_timeout_sec"),
        toml::Value::Integer(60),
    );
}

fn write_managed_skills(skills: &[SkillEntry]) -> Result<(), ApplyError> {
    let skills_root = codex_home().join("skills");
    fs::create_dir_all(&skills_root).map_err(|e| ApplyError::Io {
        context: format!("create {}", skills_root.display()),
        source: e,
    })?;

    clear_managed_skill_dirs(&skills_root)?;

    for skill in skills {
        if !safe_id_segment(skill.id.as_str()) {
            return Err(ApplyError::UnsafeSkillId(skill.id.clone()));
        }
        let dir_name = format!("{SKILL_DIR_PREFIX}{}", skill.id.as_str());
        let dir = skills_root.join(&dir_name);
        fs::create_dir_all(&dir).map_err(|e| ApplyError::Io {
            context: format!("create {}", dir.display()),
            source: e,
        })?;
        let path = dir.join("SKILL.md");
        fs::write(&path, skill_markdown(skill)).map_err(|e| ApplyError::Io {
            context: format!("write {}", path.display()),
            source: e,
        })?;
    }
    Ok(())
}

fn clear_managed_skill_dirs(skills_root: &Path) -> Result<(), ApplyError> {
    let entries = match fs::read_dir(skills_root) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(ApplyError::Io {
                context: format!("read_dir {}", skills_root.display()),
                source: e,
            });
        },
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if !name_str.starts_with(SKILL_DIR_PREFIX) {
            continue;
        }
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        fs::remove_dir_all(&path).map_err(|e| ApplyError::Io {
            context: format!("remove {}", path.display()),
            source: e,
        })?;
    }
    Ok(())
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
        return s.to_string();
    }
    let escaped = s.replace('"', "\\\"");
    format!("\"{escaped}\"")
}
