//! The `config.toml` blocks that register the marketplace and MCP connectors.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use crate::gateway::manifest::ManagedMcpServer;
use crate::sync::{ApplyError, TomlError};

use super::super::config::user_config_path;
use super::super::probe::write_dotted;
use super::{MARKETPLACE, io_err, marketplace_root, plugin_id};

pub(super) fn write_config_blocks(
    enabled: bool,
    mcp_servers: &[ManagedMcpServer],
) -> Result<(), ApplyError> {
    let path = user_config_path();
    let mut value = read_or_empty_toml(&path)?;
    let original = value.clone();

    if enabled {
        let root = marketplace_root();
        // Why: Codex stamps `last_updated` into this block, so replacing rather
        // than merging it forces a needless re-sync.
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
