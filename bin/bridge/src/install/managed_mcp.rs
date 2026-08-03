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

// Why: a read error counts as "does not match" so an unreadable file still
// triggers the write path — idempotent syncs must skip elevation, but an
// unknown on-disk state must never be mistaken for an up-to-date one.
fn body_matches(path: &Path, body: &str) -> bool {
    fs::read(path).is_ok_and(|bytes| bytes == body.as_bytes())
}

// Why: on Enforced the caller must not write per-user MCP config
// (`managed-mcp.json` suppresses it); on Unenforced it must, or MCP is absent.
// Declined = user cancelled the elevation prompt — treat like Unenforced for
// the caller but logged distinctly so operators can see the intent.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PolicyOutcome {
    Enforced,
    Unenforced,
    Declined,
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
            tracing::warn!(
                target: "bridge::install::managed-mcp",
                path = %mcp_path.display(),
                error = %e,
                "failed to render managed-mcp.json body",
            );
            return PolicyOutcome::Unenforced;
        },
    };

    let settings_path = dir.join(MANAGED_SETTINGS_FILE);
    let settings_body = match render_managed_settings(&settings_path, &servers) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(
                target: "bridge::install::managed-mcp",
                path = %settings_path.display(),
                error = %e,
                "failed to render managed-settings.json body",
            );
            return PolicyOutcome::Unenforced;
        },
    };

    // Why: diff-first — if both files already match, skip elevation entirely so
    // idempotent syncs don't prompt.
    if body_matches(&mcp_path, &mcp_body) && body_matches(&settings_path, &settings_body) {
        return PolicyOutcome::Enforced;
    }

    match write_both(&mcp_path, &mcp_body, &settings_path, &settings_body) {
        WriteOutcome::Ok => {
            tracing::info!(
                target: "bridge::install::managed-mcp",
                path = %mcp_path.display(),
                servers = servers.len(),
                "Claude Code MCP policy applied — the CLI now has exclusive control over MCP servers"
            );
            PolicyOutcome::Enforced
        },
        WriteOutcome::Declined => {
            tracing::warn!(
                target: "bridge::install::managed-mcp",
                "user declined the administrator authorization prompt; Claude Code MCP policy \
                 was not written — per-plugin .mcp.json files remain in place"
            );
            PolicyOutcome::Declined
        },
        WriteOutcome::Failed(msg) => {
            tracing::warn!(
                target: "bridge::install::managed-mcp",
                error = %msg,
                "failed to write Claude Code MCP policy — falling back to per-plugin .mcp.json"
            );
            PolicyOutcome::Unenforced
        },
    }
}

enum WriteOutcome {
    Ok,
    #[cfg_attr(
        not(target_os = "macos"),
        expect(
            dead_code,
            reason = "only the macOS elevation path can be declined by the operator"
        )
    )]
    Declined,
    Failed(String),
}

#[cfg(target_os = "macos")]
fn write_both(
    mcp_path: &Path,
    mcp_body: &str,
    settings_path: &Path,
    settings_body: &str,
) -> WriteOutcome {
    // Why: try the direct write first — CI, root shells and MDM-provisioned
    // users are already privileged and must not be prompted at all.
    let direct = write_policy_file(mcp_path, mcp_body)
        .and_then(|()| write_policy_file(settings_path, settings_body));
    if direct.is_ok() {
        return WriteOutcome::Ok;
    }
    // Why: only permission-denied justifies escalating — anything else
    // (ENOSPC and friends) is a real failure and elevation cannot fix it.
    if let Err(err) = &direct
        && err.kind() != std::io::ErrorKind::PermissionDenied
    {
        return WriteOutcome::Failed(err.to_string());
    }

    // Why: stage into a user-writable tempdir first — the elevated shell can
    // read it, whereas a heredoc would embed the body in the script itself.
    let tmp = match stage_temp(mcp_body, settings_body) {
        Ok(t) => t,
        Err(e) => return WriteOutcome::Failed(format!("stage temp: {e}")),
    };
    let script = format!(
        "set -e\n\
         /bin/mkdir -p {dir}\n\
         /usr/bin/install -m 0644 {tmp_mcp} {mcp}\n\
         /usr/bin/install -m 0644 {tmp_settings} {settings}\n",
        dir = shell_quote(&mcp_path.parent().unwrap_or(mcp_path).to_string_lossy()),
        tmp_mcp = shell_quote(&tmp.mcp.to_string_lossy()),
        mcp = shell_quote(&mcp_path.to_string_lossy()),
        tmp_settings = shell_quote(&tmp.settings.to_string_lossy()),
        settings = shell_quote(&settings_path.to_string_lossy()),
    );
    let result = crate::install::elevate::run_privileged(
        &script,
        "Astound Bridge needs administrator privileges to install the Claude Code enterprise MCP policy.",
    );
    match result {
        Ok(()) => WriteOutcome::Ok,
        Err(crate::install::elevate::ElevationError::UserCancelled) => WriteOutcome::Declined,
        Err(e) => WriteOutcome::Failed(e.to_string()),
    }
}

#[cfg(not(target_os = "macos"))]
fn write_both(
    mcp_path: &Path,
    mcp_body: &str,
    settings_path: &Path,
    settings_body: &str,
) -> WriteOutcome {
    if let Err(e) = write_policy_file(mcp_path, mcp_body) {
        return WriteOutcome::Failed(e.to_string());
    }
    if let Err(e) = write_policy_file(settings_path, settings_body) {
        return WriteOutcome::Failed(e.to_string());
    }
    WriteOutcome::Ok
}

#[cfg(target_os = "macos")]
struct TempStaging {
    _dir: tempfile::TempDir,
    mcp: PathBuf,
    settings: PathBuf,
}

#[cfg(target_os = "macos")]
fn stage_temp(mcp_body: &str, settings_body: &str) -> Result<TempStaging, std::io::Error> {
    let dir = tempfile::Builder::new()
        .prefix("astound-install-")
        .tempdir()?;
    let mcp = dir.path().join(MANAGED_MCP_FILE);
    let settings = dir.path().join(MANAGED_SETTINGS_FILE);
    fs::write(&mcp, mcp_body.as_bytes())?;
    fs::write(&settings, settings_body.as_bytes())?;
    Ok(TempStaging {
        _dir: dir,
        mcp,
        settings,
    })
}

#[cfg(target_os = "macos")]
fn shell_quote(s: &str) -> String {
    let escaped = s.replace('\'', r"'\''");
    format!("'{escaped}'")
}

// Why: removes the files rather than writing an empty server map — an empty
// managed set leaves MCP disabled entirely instead of restoring the unmanaged
// default.
pub(crate) fn clear_policy() {
    let dir = policy_dir();
    let mcp_path = dir.join(MANAGED_MCP_FILE);
    let settings_path = dir.join(MANAGED_SETTINGS_FILE);

    // Why: try the direct removal first — a privileged user, or files that
    // never existed, must not trigger an elevation prompt.
    let stripped_settings_body = if settings_path.exists() {
        read_settings(&settings_path)
            .and_then(|mut doc| {
                doc.remove("allowedMcpServers");
                doc.remove("allowManagedMcpServersOnly");
                render_pretty(&Value::Object(doc))
            })
            .ok()
    } else {
        None
    };

    let mcp_exists = mcp_path.exists();
    let direct_ok = clear_direct(&mcp_path, &settings_path, stripped_settings_body.as_deref());
    if direct_ok || (!mcp_exists && stripped_settings_body.is_none()) {
        return;
    }

    #[cfg(target_os = "macos")]
    clear_elevated(&mcp_path, &settings_path, stripped_settings_body.as_deref());
    #[cfg(not(target_os = "macos"))]
    tracing::warn!(
        target: "bridge::install::managed-mcp",
        path = %mcp_path.display(),
        "could not remove the Claude Code MCP policy — administrator privileges required"
    );
}

fn clear_direct(
    mcp_path: &Path,
    settings_path: &Path,
    stripped_settings_body: Option<&str>,
) -> bool {
    let mcp_ok = match fs::remove_file(mcp_path) {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => false,
        Err(_) => false,
    };
    let settings_ok =
        stripped_settings_body.is_none_or(|body| write_policy_file(settings_path, body).is_ok());
    mcp_ok && settings_ok
}

#[cfg(target_os = "macos")]
fn clear_elevated(mcp_path: &Path, settings_path: &Path, stripped_settings_body: Option<&str>) {
    let mut script = String::from("set -e\n");
    if mcp_path.exists() {
        script.push_str(&format!(
            "/bin/rm -f {}\n",
            shell_quote(&mcp_path.to_string_lossy())
        ));
    }
    let tmp = if let Some(body) = stripped_settings_body {
        let dir = match tempfile::Builder::new().prefix("astound-clear-").tempdir() {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(
                    target: "bridge::install::managed-mcp",
                    error = %e,
                    "could not stage the stripped managed-settings.json for elevation",
                );
                return;
            },
        };
        let staged = dir.path().join(MANAGED_SETTINGS_FILE);
        if let Err(e) = fs::write(&staged, body.as_bytes()) {
            tracing::warn!(
                target: "bridge::install::managed-mcp",
                error = %e,
                "could not stage the stripped managed-settings.json for elevation",
            );
            return;
        }
        script.push_str(&format!(
            "/usr/bin/install -m 0644 {} {}\n",
            shell_quote(&staged.to_string_lossy()),
            shell_quote(&settings_path.to_string_lossy()),
        ));
        Some(dir)
    } else {
        None
    };
    _ = tmp;
    match crate::install::elevate::run_privileged(
        &script,
        "Astound Bridge needs administrator privileges to remove the Claude Code enterprise MCP policy.",
    ) {
        Ok(()) => tracing::info!(
            target: "bridge::install::managed-mcp",
            "Claude Code MCP policy removed"
        ),
        Err(crate::install::elevate::ElevationError::UserCancelled) => tracing::warn!(
            target: "bridge::install::managed-mcp",
            "user declined the administrator authorization prompt; Claude Code MCP policy \
             files were left in place"
        ),
        Err(e) => tracing::warn!(
            target: "bridge::install::managed-mcp",
            error = %e,
            "failed to remove Claude Code MCP policy"
        ),
    }
}
