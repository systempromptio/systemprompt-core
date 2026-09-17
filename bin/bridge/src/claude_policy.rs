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
