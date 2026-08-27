//! JSON bodies for `managed-mcp.json` and the `managed-settings.json` keys the
//! bridge owns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use serde_json::{Map, Value, json};

// Why: allowlisted by URL — the CLI documents `serverName` matching as not
// being a security control.
fn allowlist_entries(servers: &Map<String, Value>) -> Vec<Value> {
    servers
        .values()
        .filter_map(|s| s.get("url").and_then(Value::as_str))
        .map(|url| json!({ "serverUrl": url }))
        .collect()
}

pub(super) fn render_pretty(doc: &Value) -> Result<String, std::io::Error> {
    Ok(format!("{}\n", serde_json::to_string_pretty(doc)?))
}

pub(super) fn render_managed_mcp(servers: &Map<String, Value>) -> Result<String, std::io::Error> {
    render_pretty(&json!({ "mcpServers": Value::Object(servers.clone()) }))
}

// Why: an unreadable existing document is an error — the file is admin-owned
// and overwriting it would clobber keys we did not author.
pub(super) fn read_settings(path: &Path) -> Result<Map<String, Value>, std::io::Error> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(e) => return Err(e),
    };
    match serde_json::from_slice::<Value>(&bytes)? {
        Value::Object(o) => Ok(o),
        _ => Err(std::io::Error::other("existing file is not a JSON object")),
    }
}

pub(super) fn render_managed_settings(
    path: &Path,
    servers: &Map<String, Value>,
    allow_claude_ai_connectors: bool,
) -> Result<String, std::io::Error> {
    let mut doc = read_settings(path)?;
    doc.insert(
        "allowedMcpServers".to_owned(),
        Value::Array(allowlist_entries(servers)),
    );
    doc.insert("allowManagedMcpServersOnly".to_owned(), Value::Bool(true));
    // Why: the key must be removed (not set false) when policy withdraws it —
    // leaving a stale `true` behind would keep connectors enabled forever.
    if allow_claude_ai_connectors {
        doc.insert("allowAllClaudeAiMcps".to_owned(), Value::Bool(true));
    } else {
        doc.remove("allowAllClaudeAiMcps");
    }
    render_pretty(&Value::Object(doc))
}
