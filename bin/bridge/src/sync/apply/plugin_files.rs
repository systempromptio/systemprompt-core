//! Reading what a promoted plugin tree carries: its bundled MCP servers, and
//! the plugin directories the manifest no longer names.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(serde::Deserialize)]
struct McpFileProbe {
    // JSON: `.mcp.json` is Claude Code's own file; only the server names are
    // read, the per-server bodies are opaque here.
    #[serde(rename = "mcpServers", default)]
    mcp_servers: BTreeMap<String, serde_json::Value>,
}

// Why: Claude Desktop would register a bundled `.mcp.json` verbatim, bypassing
// the loopback proxy; the names are recorded and the file is stripped only
// once it has parsed — a malformed file stays in place for the operator to see.
pub(super) fn extract_mcp_servers(plugin_dir: &Path) -> Result<Vec<String>, super::ApplyError> {
    let path = plugin_dir.join(".mcp.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(super::ApplyError::Io {
                context: format!("read {}", path.display()),
                source,
            });
        },
    };
    let names: Vec<String> = serde_json::from_slice::<McpFileProbe>(&bytes)
        .map(|f| f.mcp_servers.into_keys().collect())
        .map_err(|e| super::ApplyError::Serialize {
            what: format!("{} is not valid JSON", path.display()),
            source: e,
        })?;
    crate::fsutil::remove_verified(&path).map_err(|source| super::ApplyError::Io {
        context: format!("strip bundled {}", path.display()),
        source,
    })?;
    Ok(names)
}

pub(super) fn remove_stale(root: &Path, expected: &HashSet<&str>) -> Result<Vec<String>, super::ApplyError> {
    let mut removed = Vec::new();
    let entries = fs::read_dir(root).map_err(|source| super::ApplyError::Io {
        context: format!("enumerate {}", root.display()),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| super::ApplyError::Io {
            context: format!("enumerate {}", root.display()),
            source,
        })?;
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if name_str.starts_with('.') {
            continue;
        }
        if !expected.contains(name_str) && entry.path().is_dir() {
            fs::remove_dir_all(entry.path()).map_err(|e| super::ApplyError::Io {
                context: format!("remove stale {name_str}"),
                source: e,
            })?;
            removed.push(name_str.to_owned());
        }
    }
    Ok(removed)
}
