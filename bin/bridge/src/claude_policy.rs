//! Claude Code enterprise policy paths and managed-key removal.
//!
//! Shared by installation and validation to identify bridge-managed settings.
//! An MCP policy is the bridge's only when every server it names is the
//! bridge's own loopback MCP endpoint (`http://127.0.0.1:<port>/mcp/<slug>`),
//! the only shape a bridge ever wrote; any other policy belongs to whoever
//! manages the machine and is never removed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

pub const MANAGED_MCP_FILE: &str = "managed-mcp.json";
pub const MANAGED_SETTINGS_FILE: &str = "managed-settings.json";

const BRIDGE_KEYS: [&str; 3] = [
    "allowedMcpServers",
    "allowManagedMcpServersOnly",
    "allowAllClaudeAiMcps",
];

// JSON: Claude Code managed `settings.json` — foreign config, unknown keys must
// be preserved.
fn read_settings(path: &Path) -> Result<Option<Map<String, Value>>, std::io::Error> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(crate::fsutil::io_context(path.display().to_string(), e)),
    };
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(None);
    }
    let doc = serde_json::from_slice::<Value>(&bytes).map_err(|e| {
        crate::fsutil::io_context(
            path.display().to_string(),
            std::io::Error::new(std::io::ErrorKind::InvalidData, e),
        )
    })?;
    match doc {
        Value::Object(o) => Ok(Some(o)),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{}: existing file is not a JSON object", path.display()),
        )),
    }
}

#[must_use]
pub fn is_blank_file(path: &Path) -> bool {
    fs::read(path).is_ok_and(|bytes| bytes.iter().all(u8::is_ascii_whitespace))
}

/// Who wrote a Claude Code MCP policy file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyAuthor {
    Bridge,
    Foreign,
}

#[must_use]
pub fn is_bridge_loopback_mcp_url(raw: &str) -> bool {
    url::Url::parse(raw).is_ok_and(|url| {
        url.scheme() == "http"
            && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
            && systemprompt_models::ApiPaths::mcp_server_slug(url.path()).is_some()
    })
}

// JSON: Claude Code `managed-mcp.json` — foreign config whose author is judged
// from its server URLs.
fn servers_are_bridge_loopback(doc: &Value) -> bool {
    doc.get("mcpServers")
        .and_then(Value::as_object)
        .is_some_and(|servers| {
            !servers.is_empty()
                && servers.values().all(|server| {
                    server
                        .get("url")
                        .and_then(Value::as_str)
                        .is_some_and(is_bridge_loopback_mcp_url)
                })
        })
}

pub fn managed_mcp_author(path: &Path) -> Result<Option<PolicyAuthor>, std::io::Error> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(crate::fsutil::io_context(path.display().to_string(), e)),
    };
    let ours =
        serde_json::from_slice::<Value>(&bytes).is_ok_and(|doc| servers_are_bridge_loopback(&doc));
    Ok(Some(if ours {
        PolicyAuthor::Bridge
    } else {
        PolicyAuthor::Foreign
    }))
}

// JSON: Claude Code managed `settings.json` — the allowlist is the bridge's
// only when every entry names a bridge loopback MCP URL.
fn allowlist_is_bridge_loopback(doc: &Map<String, Value>) -> bool {
    doc.get("allowedMcpServers")
        .and_then(Value::as_array)
        .is_some_and(|entries| {
            !entries.is_empty()
                && entries.iter().all(|entry| {
                    entry
                        .get("serverUrl")
                        .and_then(Value::as_str)
                        .is_some_and(is_bridge_loopback_mcp_url)
                })
        })
}

#[must_use]
pub fn holds_foreign_mcp_keys(path: &Path) -> bool {
    read_settings(path).ok().flatten().is_some_and(|doc| {
        BRIDGE_KEYS.iter().any(|key| doc.contains_key(*key)) && !allowlist_is_bridge_loopback(&doc)
    })
}

pub fn stripped_settings(path: &Path) -> Result<Option<String>, std::io::Error> {
    let Some(mut doc) = read_settings(path)? else {
        return Ok(None);
    };
    if !allowlist_is_bridge_loopback(&doc) {
        return Ok(None);
    }
    for key in BRIDGE_KEYS {
        doc.remove(key);
    }
    Ok(Some(format!(
        "{}\n",
        serde_json::to_string_pretty(&Value::Object(doc))?
    )))
}

/// One `allowedWorkspaceFolders` entry in the shape Claude Desktop validates:
/// a folder path, an optional access mode and an optional boolean
/// pre-selection flag. Anything else in an entry is malformed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceFolder {
    pub path: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default, rename = "isDefaultSelected")]
    pub is_default_selected: Option<bool>,
}

/// The outcome of validating a published `allowedWorkspaceFolders` value the
/// way Claude Desktop does: the entries it keeps and, per dropped entry, the
/// JSON text and the reason.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceFoldersAudit {
    pub kept: Vec<WorkspaceFolder>,
    pub dropped: Vec<(String, String)>,
}

impl WorkspaceFoldersAudit {
    #[must_use]
    pub const fn blocks_all_folders(&self) -> bool {
        self.kept.is_empty()
    }

    #[must_use]
    pub fn allows_home(&self) -> bool {
        self.kept.iter().any(|f| f.path == "~")
    }
}

// Why: Claude Desktop drops a malformed entry and keeps going, and an empty
// resulting list blocks every folder, so the audit reports per entry rather
// than failing the list on the first bad one.
pub fn audit_workspace_folders(raw: &str) -> Result<WorkspaceFoldersAudit, serde_json::Error> {
    let entries: Vec<Value> = serde_json::from_str(raw)?;
    let mut audit = WorkspaceFoldersAudit::default();
    for entry in entries {
        let folder = match &entry {
            Value::String(path) => Ok(WorkspaceFolder {
                path: path.clone(),
                mode: None,
                is_default_selected: None,
            }),
            other => serde_json::from_value::<WorkspaceFolder>(other.clone()),
        };
        match folder {
            Ok(folder) if folder.path.trim().is_empty() => {
                audit
                    .dropped
                    .push((entry.to_string(), "empty path".to_owned()));
            },
            Ok(folder) => audit.kept.push(folder),
            Err(e) => audit.dropped.push((entry.to_string(), e.to_string())),
        }
    }
    Ok(audit)
}
