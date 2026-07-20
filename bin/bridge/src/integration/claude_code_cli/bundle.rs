//! Mirrors synced org-plugin trees into the CLI's marketplace layout and
//! re-projects each plugin's managed MCP servers through the bridge proxy as a
//! plugin-local `.mcp.json`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use serde_json::{Map, json};

use super::io_err;
use crate::sync::ApplyError;

pub(super) fn mirror_plugin(
    src: &Path,
    dst: &Path,
    mcp_servers: &[String],
) -> Result<(), ApplyError> {
    if dst.exists() {
        fs::remove_dir_all(dst).map_err(|e| io_err(format!("clear {}", dst.display()), e))?;
    }
    copy_dir_all(src, dst)?;
    if !mcp_servers.is_empty() {
        write_mcp_json(dst, mcp_servers)?;
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), ApplyError> {
    fs::create_dir_all(dst).map_err(|e| io_err(format!("create {}", dst.display()), e))?;
    let entries =
        fs::read_dir(src).map_err(|e| io_err(format!("read dir {}", src.display()), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| io_err(format!("read dir {}", src.display()), e))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|e| io_err(format!("stat {}", from.display()), e))?;
        if file_type.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to)
                .map_err(|e| io_err(format!("copy {} -> {}", from.display(), to.display()), e))?;
        }
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

pub(super) fn remove_stale_children(dir: &Path, expected: &[&str]) -> Result<(), ApplyError> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if name_str.starts_with('.') || expected.contains(&name_str) {
            continue;
        }
        if entry.path().is_dir() {
            remove_dir(&entry.path())?;
        }
    }
    Ok(())
}

fn write_mcp_json(root: &Path, servers: &[String]) -> Result<(), ApplyError> {
    let bearer = crate::proxy::loopback_bearer()
        .map_err(|e| io_err("read loopback secret for claude-code .mcp.json", e))?;
    let mut map = Map::new();
    for name in servers {
        let slug = crate::mcp_registry::normalize_key(name);
        map.insert(
            slug.clone(),
            json!({
                "type": "http",
                "url": crate::proxy::mcp_url(&slug),
                "headers": { "Authorization": bearer.clone() },
            }),
        );
    }
    super::json_io::write_json(&root.join(".mcp.json"), &json!({ "mcpServers": map }))
}
