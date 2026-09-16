//! The `config.toml` blocks that register the marketplace and MCP connectors,
//! edited in place so the user's own keys, comments and layout survive.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use toml_edit::{DocumentMut, Item};

use crate::config::write::{ConfigWriteError, remove, set};
use crate::gateway::manifest::ManagedMcpServer;
use crate::host_sync::{ApplyError, TomlError};
use crate::proxy::LoopbackEndpoint;

use super::super::config::user_config_path;
use super::{MARKETPLACE, io_err, marketplace_root, plugin_id};

fn edit_err(path: &Path, e: ConfigWriteError) -> ApplyError {
    ApplyError::Io {
        context: format!("edit {}", path.display()),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    }
}

pub(super) fn write_config_blocks(
    loopback: &LoopbackEndpoint,
    enabled: bool,
    mcp_servers: &[ManagedMcpServer],
) -> Result<(), ApplyError> {
    let path = user_config_path();
    let mut doc = read_or_empty_toml(&path)?;
    let original = doc.to_string();

    if enabled {
        let root = marketplace_root();
        let plugin = plugin_id();
        // Why: Codex stores last_updated here; replacing the block triggers another
        // sync.
        set(
            &mut doc,
            &["marketplaces", MARKETPLACE, "source_type"],
            "local",
        )
        .map_err(|e| edit_err(&path, e))?;
        set(
            &mut doc,
            &["marketplaces", MARKETPLACE, "source"],
            root.display().to_string(),
        )
        .map_err(|e| edit_err(&path, e))?;
        set(&mut doc, &["plugins", plugin.as_str(), "enabled"], true)
            .map_err(|e| edit_err(&path, e))?;
    } else {
        remove_marketplace_registration(&mut doc, &path)?;
    }

    let sidecar = crate::integration::mcp_sidecar::beside(&path);
    strip_bridge_mcp_servers(&crate::integration::mcp_sidecar::read(&sidecar)?, &mut doc);
    let written = if enabled {
        write_mcp_servers(loopback, &mut doc, mcp_servers, &path)?
    } else {
        Vec::new()
    };
    crate::integration::mcp_sidecar::write(&sidecar, &written)?;

    if doc.to_string() == original {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io_err("create config dir", parent, e))?;
    }
    crate::fsutil::atomic_write_0644(&path, doc.to_string().as_bytes())
        .map_err(|e| io_err("write config.toml", &path, e))
}

fn write_mcp_servers(
    loopback: &LoopbackEndpoint,
    doc: &mut DocumentMut,
    servers: &[ManagedMcpServer],
    path: &Path,
) -> Result<Vec<String>, ApplyError> {
    if servers.is_empty() {
        return Ok(Vec::new());
    }
    let mut written = Vec::with_capacity(servers.len());
    let bearer = loopback
        .host_bearer(&crate::ids::HostId::new("codex-cli"))
        .map_err(|e| ApplyError::Io {
            context: "derive codex host token for mcp_servers".into(),
            source: e,
        })?;
    for s in servers {
        let slug = crate::mcp_registry::normalize_key(s.name.as_str());
        set(doc, &["mcp_servers", &slug, "url"], loopback.mcp_url(&slug))
            .map_err(|e| edit_err(path, e))?;
        set(
            doc,
            &["mcp_servers", &slug, "http_headers", "Authorization"],
            bearer.as_str(),
        )
        .map_err(|e| edit_err(path, e))?;
        written.push(slug);
    }
    Ok(written)
}

fn read_or_empty_toml(path: &Path) -> Result<DocumentMut, ApplyError> {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(io_err("read config.toml", path, e)),
    };
    raw.parse::<DocumentMut>().map_err(|e| ApplyError::Toml {
        what: format!("parse {}", path.display()),
        source: TomlError::from(Box::new(e)),
    })
}

fn remove_marketplace_registration(doc: &mut DocumentMut, path: &Path) -> Result<(), ApplyError> {
    let plugin = plugin_id();
    remove(doc, &["plugins", plugin.as_str()]).map_err(|e| edit_err(path, e))?;
    remove(doc, &["marketplaces", MARKETPLACE]).map_err(|e| edit_err(path, e))?;
    prune_empty_table(doc, "plugins");
    prune_empty_table(doc, "marketplaces");
    Ok(())
}

fn strip_bridge_mcp_servers(recorded: &[String], doc: &mut DocumentMut) {
    if let Some(servers) = doc.get_mut("mcp_servers").and_then(Item::as_table_mut) {
        for name in recorded {
            servers.remove(name);
        }
    }
    prune_empty_table(doc, "mcp_servers");
}

fn prune_empty_table(doc: &mut DocumentMut, key: &str) {
    if doc
        .get(key)
        .and_then(Item::as_table)
        .is_some_and(toml_edit::Table::is_empty)
    {
        doc.remove(key);
    }
}
