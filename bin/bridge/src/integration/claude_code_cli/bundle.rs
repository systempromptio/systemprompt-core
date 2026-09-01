//! Mirrors synced org-plugin trees into the CLI's marketplace layout, and
//! provisions MCP servers per-plugin when the elevated policy is unavailable.
//!
//! The global `managed-mcp.json` written by [`crate::install::managed_mcp`]
//! suppresses plugin-provided servers, so the plugin-local `.mcp.json` here is
//! written only when that elevated policy could not be applied — otherwise an
//! unprivileged install would finish with no MCP servers at all.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use serde_json::json;

use super::io_err;
use crate::host_sync::ApplyError;
use crate::proxy::LoopbackEndpoint;

pub(super) fn mirror_plugin(
    loopback: &LoopbackEndpoint,
    src: &Path,
    dst: &Path,
    mcp_servers: &[String],
) -> Result<(), ApplyError> {
    if dst.exists() {
        fs::remove_dir_all(dst).map_err(|e| io_err(format!("clear {}", dst.display()), e))?;
    }
    copy_dir_all(src, dst)?;
    drop_standard_hooks_pointer(dst)?;
    if !mcp_servers.is_empty() {
        write_mcp_json(loopback, dst, mcp_servers)?;
    }
    Ok(())
}

// Why: the Claude Code CLI loads `hooks/hooks.json` itself and then rejects any
// plugin whose manifest also points at it ("Duplicate hooks file detected").
fn drop_standard_hooks_pointer(dst: &Path) -> Result<(), ApplyError> {
    const STANDARD: [&str; 2] = ["./hooks/hooks.json", "hooks/hooks.json"];
    let path = dst.join(".claude-plugin").join("plugin.json");
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            tracing::warn!(
                target: "bridge::claude-code-cli",
                path = %path.display(),
                error = %e,
                "plugin manifest unreadable; leaving its hooks pointer untouched"
            );
            return Ok(());
        },
    };
    let Ok(serde_json::Value::Object(mut manifest)) = serde_json::from_str(&text) else {
        tracing::warn!(
            target: "bridge::claude-code-cli",
            path = %path.display(),
            "plugin manifest is not a JSON object; leaving its hooks pointer untouched"
        );
        return Ok(());
    };
    let points_at_standard = manifest
        .get("hooks")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|h| STANDARD.contains(&h));
    if !points_at_standard {
        return Ok(());
    }
    manifest.remove("hooks");
    let bytes = serde_json::to_vec_pretty(&serde_json::Value::Object(manifest)).map_err(|e| {
        ApplyError::Serialize {
            what: path.display().to_string(),
            source: e,
        }
    })?;
    fs::write(&path, &bytes).map_err(|e| io_err(format!("write {}", path.display()), e))?;
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

fn write_mcp_json(
    loopback: &LoopbackEndpoint,
    root: &Path,
    servers: &[String],
) -> Result<(), ApplyError> {
    let bearer = loopback
        .bearer()
        .map_err(|e| io_err("read loopback secret for claude-code .mcp.json", e))?;
    let mut map = serde_json::Map::new();
    for name in servers {
        let slug = crate::mcp_registry::normalize_key(name);
        map.insert(
            slug.clone(),
            json!({
                "type": "http",
                "url": loopback.mcp_url(&slug),
                "headers": { "Authorization": bearer.clone() },
            }),
        );
    }
    super::json_io::write_json(&root.join(".mcp.json"), &json!({ "mcpServers": map }))
}

pub(super) fn remove_stale_children(dir: &Path, expected: &[&str]) -> Result<(), ApplyError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            tracing::warn!(
                target: "bridge::claude-code-cli",
                dir = %dir.display(),
                error = %e,
                "marketplace directory unreadable; stale plugins left in place"
            );
            return Ok(());
        },
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
