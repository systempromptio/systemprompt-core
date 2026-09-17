//! Claude Code enterprise policy paths and managed-key removal.
//!
//! Shared by installation and validation to identify bridge-managed settings.
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

fn read_settings(path: &Path) -> Result<Option<Map<String, Value>>, std::io::Error> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(std::io::Error::new(
                e.kind(),
                format!("{}: {e}", path.display()),
            ));
        },
    };
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(None);
    }
    let doc = serde_json::from_slice::<Value>(&bytes).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{}: {e}", path.display()),
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

pub fn stripped_settings(path: &Path) -> Result<Option<String>, std::io::Error> {
    let Some(mut doc) = read_settings(path)? else {
        return Ok(None);
    };
    let mut changed = false;
    for key in BRIDGE_KEYS {
        changed |= doc.remove(key).is_some();
    }
    if !changed {
        return Ok(None);
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
