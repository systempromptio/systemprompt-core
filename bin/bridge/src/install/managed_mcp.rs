//! Claude Code CLI enterprise MCP policy: `managed-mcp.json` plus the
//! `allowedMcpServers` allowlist in `managed-settings.json`.
//!
//! This is a different surface from [`crate::install::mdm`], which writes the
//! Claude Desktop `managedMcpServers` policy value (an array, under a registry
//! key or plist). The CLI reads a standalone JSON file at a fixed system path
//! and, once it exists, loads *only* the servers it names — plugin-provided
//! servers and claude.ai connectors are suppressed.
//!
//! Both files live in a system directory and need elevation to write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

const MANAGED_MCP_FILE: &str = "managed-mcp.json";
const MANAGED_SETTINGS_FILE: &str = "managed-settings.json";

#[must_use]
pub(crate) fn policy_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/Library/Application Support/ClaudeCode")
    }
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"C:\Program Files\ClaudeCode")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        PathBuf::from("/etc/claude-code")
    }
}

pub(crate) fn server_map() -> Result<Map<String, Value>, std::io::Error> {
    let registry = crate::mcp_registry::snapshot();
    let bearer = crate::proxy::loopback_bearer()?;
    let mut slugs: Vec<&String> = registry.keys().collect();
    slugs.sort();
    let mut map = Map::new();
    for slug in slugs {
        map.insert(
            slug.clone(),
            json!({
                "type": "http",
                "url": crate::proxy::mcp_url(slug.as_str()),
                "headers": { "Authorization": bearer.clone() },
            }),
        );
    }
    Ok(map)
}

// Why: allowlisted by URL — the CLI documents `serverName` matching as not
// being a security control.
fn allowlist_entries(servers: &Map<String, Value>) -> Vec<Value> {
    servers
        .values()
        .filter_map(|s| s.get("url").and_then(Value::as_str))
        .map(|url| json!({ "serverUrl": url }))
        .collect()
}

fn render_pretty(doc: &Value) -> Result<String, std::io::Error> {
    Ok(format!("{}\n", serde_json::to_string_pretty(doc)?))
}

fn render_managed_mcp(servers: &Map<String, Value>) -> Result<String, std::io::Error> {
    render_pretty(&json!({ "mcpServers": Value::Object(servers.clone()) }))
}

// Why: an unreadable existing document is an error — the file is admin-owned
// and overwriting it would clobber keys we did not author.
fn read_settings(path: &Path) -> Result<Map<String, Value>, std::io::Error> {
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

fn render_managed_settings(
    path: &Path,
    servers: &Map<String, Value>,
) -> Result<String, std::io::Error> {
    let mut doc = read_settings(path)?;
    doc.insert(
        "allowedMcpServers".to_owned(),
        Value::Array(allowlist_entries(servers)),
    );
    doc.insert("allowManagedMcpServersOnly".to_owned(), Value::Bool(true));
    render_pretty(&Value::Object(doc))
}

fn write_policy_file(path: &Path, body: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, body.as_bytes())
}

fn report_unwritable(path: &Path, err: &std::io::Error, body: &str) {
    tracing::warn!(
        target: "bridge::install::managed-mcp",
        path = %path.display(),
        error = %err,
        file_body = %body,
        "could not write Claude Code MCP policy — it needs administrator privileges, so MCP \
         servers are being provisioned per-user instead and users remain free to add their \
         own. To enforce the managed set, deploy `file_body` at `path` via MDM, Group Policy \
         or `sudo`"
    );
}

// Why: on Enforced the caller must not write per-user MCP config
// (`managed-mcp.json` suppresses it); on Unenforced it must, or MCP is absent.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PolicyOutcome {
    Enforced,
    Unenforced,
}

pub(crate) fn apply_policy() -> PolicyOutcome {
    let dir = policy_dir();
    let servers = match server_map() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                target: "bridge::install::managed-mcp",
                error = %e,
                "loopback secret unavailable; leaving the existing MCP policy in place"
            );
            return PolicyOutcome::Unenforced;
        },
    };

    let mcp_path = dir.join(MANAGED_MCP_FILE);
    let mcp_body = match render_managed_mcp(&servers) {
        Ok(b) => b,
        Err(e) => {
            report_unwritable(&mcp_path, &e, "");
            return PolicyOutcome::Unenforced;
        },
    };
    if let Err(e) = write_policy_file(&mcp_path, &mcp_body) {
        report_unwritable(&mcp_path, &e, &mcp_body);
        return PolicyOutcome::Unenforced;
    }

    let settings_path = dir.join(MANAGED_SETTINGS_FILE);
    let settings_body = match render_managed_settings(&settings_path, &servers) {
        Ok(b) => b,
        Err(e) => {
            report_unwritable(&settings_path, &e, "");
            return PolicyOutcome::Unenforced;
        },
    };
    if let Err(e) = write_policy_file(&settings_path, &settings_body) {
        report_unwritable(&settings_path, &e, &settings_body);
        return PolicyOutcome::Unenforced;
    }

    tracing::info!(
        target: "bridge::install::managed-mcp",
        path = %mcp_path.display(),
        servers = servers.len(),
        "Claude Code MCP policy applied — the CLI now has exclusive control over MCP servers"
    );
    PolicyOutcome::Enforced
}

// Why: removes the files rather than writing an empty server map — an empty
// managed set leaves MCP disabled entirely instead of restoring the unmanaged
// default.
pub(crate) fn clear_policy() {
    let dir = policy_dir();
    let mcp_path = dir.join(MANAGED_MCP_FILE);
    if let Err(e) = fs::remove_file(&mcp_path)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(
            target: "bridge::install::managed-mcp",
            path = %mcp_path.display(),
            error = %e,
            "could not remove the Claude Code MCP policy; it needs administrator privileges"
        );
    }

    let settings_path = dir.join(MANAGED_SETTINGS_FILE);
    if !settings_path.exists() {
        return;
    }
    let stripped = read_settings(&settings_path).and_then(|mut doc| {
        doc.remove("allowedMcpServers");
        doc.remove("allowManagedMcpServersOnly");
        let body = render_pretty(&Value::Object(doc))?;
        write_policy_file(&settings_path, &body)
    });
    if let Err(e) = stripped {
        tracing::warn!(
            target: "bridge::install::managed-mcp",
            path = %settings_path.display(),
            error = %e,
            "could not strip the MCP allowlist from managed settings"
        );
    }
}
