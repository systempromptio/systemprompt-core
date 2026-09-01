//! The `config.yaml` blocks that register MCP connectors and the managed-skills
//! external directory. Preserves every user-authored key, including the `model`
//! block the installer owns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use serde_yaml::Value;

use crate::gateway::manifest::ManagedMcpServer;
use crate::host_sync::ApplyError;

use super::super::config::{config_yaml_path, skills_dir};
use super::super::probe::write_dotted;
use super::io_err;

const MCP_TABLE: &str = "mcp_servers";
const TRANSPORT_STREAMABLE: &str = "streamable";

fn key(k: &str) -> Value {
    Value::String(k.to_owned())
}

fn yaml_err(what: &str, e: serde_yaml::Error) -> ApplyError {
    ApplyError::Io {
        context: format!("hermes config.yaml: {what}"),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    }
}

pub(super) fn write_config_blocks(
    enabled: bool,
    mcp_servers: &[ManagedMcpServer],
) -> Result<(), ApplyError> {
    let path = config_yaml_path();
    let mut value = read_or_empty(&path)?;
    let original = value.clone();

    strip_bridge_mcp_servers(&mut value);
    remove_external_dir(&mut value);

    if enabled {
        add_external_dir(&mut value);
        write_mcp_servers(&mut value, mcp_servers)?;
    }

    if value == original {
        return Ok(());
    }

    let rendered = serde_yaml::to_string(&value).map_err(|e| yaml_err("serialize", e))?;
    crate::fsutil::atomic_write_0600(&path, rendered.as_bytes())
        .map_err(|e| io_err("write config.yaml", &path, e))
}

fn write_mcp_servers(value: &mut Value, servers: &[ManagedMcpServer]) -> Result<(), ApplyError> {
    if servers.is_empty() {
        return Ok(());
    }
    let bearer = crate::proxy::loopback_bearer().map_err(|e| ApplyError::Io {
        context: "read loopback secret for hermes mcp_servers".into(),
        source: e,
    })?;
    for s in servers {
        let slug = crate::mcp_registry::normalize_key(s.name.as_str());
        write_dotted(
            value,
            &format!("{MCP_TABLE}.{slug}.url"),
            Value::String(crate::proxy::mcp_url(&slug)),
        );
        write_dotted(
            value,
            &format!("{MCP_TABLE}.{slug}.headers.Authorization"),
            Value::String(bearer.clone()),
        );
        write_dotted(
            value,
            &format!("{MCP_TABLE}.{slug}.transport"),
            Value::String(TRANSPORT_STREAMABLE.to_owned()),
        );
    }
    Ok(())
}

fn read_or_empty(path: &Path) -> Result<Value, ApplyError> {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(io_err("read config.yaml", path, e)),
    };
    if raw.trim().is_empty() {
        return Ok(Value::Mapping(serde_yaml::Mapping::new()));
    }
    let value: Value = serde_yaml::from_str(&raw).map_err(|e| yaml_err("parse", e))?;
    if matches!(value, Value::Mapping(_)) {
        Ok(value)
    } else {
        Ok(Value::Mapping(serde_yaml::Mapping::new()))
    }
}

fn strip_bridge_mcp_servers(root: &mut Value) {
    let Value::Mapping(top) = root else {
        return;
    };
    let Some(Value::Mapping(servers)) = top.get_mut(key(MCP_TABLE)) else {
        return;
    };
    let prefix = format!("{}/mcp/", crate::proxy::loopback_origin());
    let ours: Vec<Value> = servers
        .iter()
        .filter_map(|(name, entry)| {
            let is_ours = entry
                .get("url")
                .and_then(Value::as_str)
                .is_some_and(|u| u.starts_with(&prefix));
            is_ours.then(|| name.clone())
        })
        .collect();
    for name in ours {
        servers.remove(name);
    }
    if servers.is_empty() {
        top.remove(key(MCP_TABLE));
    }
}

fn external_dir_value() -> Value {
    Value::String(skills_dir().display().to_string())
}

fn add_external_dir(root: &mut Value) {
    let want = external_dir_value();
    write_dotted_seq(root, &want);
}

// Why: `skills.external_dirs` is a list; append the managed dir only if absent
// so re-applies stay idempotent and user-added dirs are preserved.
fn write_dotted_seq(root: &mut Value, want: &Value) {
    let Value::Mapping(top) = root else {
        return;
    };
    let skills = top
        .entry(key("skills"))
        .or_insert_with(|| Value::Mapping(serde_yaml::Mapping::new()));
    if !matches!(skills, Value::Mapping(_)) {
        *skills = Value::Mapping(serde_yaml::Mapping::new());
    }
    let Value::Mapping(skills_map) = skills else {
        return;
    };
    let dirs = skills_map
        .entry(key("external_dirs"))
        .or_insert_with(|| Value::Sequence(Vec::new()));
    if !matches!(dirs, Value::Sequence(_)) {
        *dirs = Value::Sequence(Vec::new());
    }
    if let Value::Sequence(seq) = dirs
        && !seq.iter().any(|v| v == want)
    {
        seq.push(want.clone());
    }
}

fn remove_external_dir(root: &mut Value) {
    let want = external_dir_value();
    let Value::Mapping(top) = root else {
        return;
    };
    let Some(Value::Mapping(skills_map)) = top.get_mut(key("skills")) else {
        return;
    };
    if let Some(Value::Sequence(seq)) = skills_map.get_mut(key("external_dirs")) {
        seq.retain(|v| v != &want);
        if seq.is_empty() {
            skills_map.remove(key("external_dirs"));
        }
    }
    if skills_map.is_empty() {
        top.remove(key("skills"));
    }
}
