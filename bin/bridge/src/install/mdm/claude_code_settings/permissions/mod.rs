//! `permissions.allow` / `permissions.deny` rules for Claude Code: the
//! decision each managed MCP server's manifest entry carries, written into
//! the settings files the bridge owns.
//!
//! A managed server is one the operator provisioned and the gateway's
//! governance chain already judges every call to, so the client's own
//! per-call prompt is noise rather than control. The manifest says `allow`
//! for such a server by default (`ManagedMcpServer::default_tool_policy`),
//! and this module turns that into the `mcp__<server>` rules Claude Code
//! reads — plus the `mcp__plugin_<plugin>_<server>` spelling for a server a
//! plugin's `.mcp.json` mirrors. Rules the bridge wrote last time are
//! recorded in a sidecar so a server that leaves the manifest, or flips to
//! `prompt`, has its rules taken back out; rules the user added stay. Rules
//! land in the managed settings file (the machine policy file when writable,
//! else the user's `settings.json`) and the standalone file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    io_error, managed_settings_path, read_or_empty, render, standalone_settings_path, write_atomic,
};
use crate::gateway::manifest::SignedManifest;
use crate::install::mdm::MdmError;

mod merge;

pub use merge::merged_permissions;

const SIDECAR: &str = "claude-code-permissions.json";

/// The rules the bridge derives from one manifest, split the way Claude
/// Code's `permissions` object is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRules {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}

impl PermissionRules {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.allow.is_empty() && self.deny.is_empty()
    }
}

fn server_prefixes(server: &str, plugins: &[&str]) -> Vec<String> {
    let mut out = vec![format!("mcp__{server}")];
    out.extend(
        plugins
            .iter()
            .map(|plugin| format!("mcp__plugin_{plugin}_{server}")),
    );
    out
}

#[must_use]
pub fn rules_for(
    manifest: &SignedManifest,
    plugin_mcp_servers: &std::collections::BTreeMap<String, Vec<String>>,
) -> PermissionRules {
    use systemprompt_models::bridge::ids::ToolPolicy;
    use systemprompt_models::bridge::manifest::ManagedMcpServer;

    let mut rules = PermissionRules::default();
    for server in &manifest.managed_mcp_servers {
        let name = server.name.as_str();
        let plugins: Vec<&str> = plugin_mcp_servers
            .iter()
            .filter(|(_, names)| names.iter().any(|n| n == name))
            .map(|(plugin, _)| plugin.as_str())
            .collect();
        let prefixes = server_prefixes(name, &plugins);
        let Some(policies) = &server.tool_policy else {
            continue;
        };
        // Why: Claude Code's deny list beats its allow list, so under a
        // wildcard deny a named allow is dead and is not written.
        let denied_outright = server.default_tool_policy() == Some(ToolPolicy::Deny);
        for (tool, policy) in policies {
            if denied_outright && *policy == ToolPolicy::Allow {
                continue;
            }
            let bucket = match policy {
                ToolPolicy::Allow => &mut rules.allow,
                ToolPolicy::Deny => &mut rules.deny,
                ToolPolicy::Prompt => continue,
            };
            for prefix in &prefixes {
                if tool.as_str() == ManagedMcpServer::TOOL_POLICY_WILDCARD {
                    bucket.push(prefix.clone());
                } else {
                    bucket.push(format!("{prefix}__{tool}"));
                }
            }
        }
    }
    rules.allow.sort();
    rules.allow.dedup();
    rules.deny.sort();
    rules.deny.dedup();
    rules
}

fn sidecar_path() -> Option<PathBuf> {
    crate::config::paths::bridge_metadata_dir().map(|d| d.join(SIDECAR))
}

// Why: the sidecar is the only record of which rules the bridge wrote; a
// read or parse failure must surface, or those rules are orphaned in the
// user's settings rather than replaced.
fn read_sidecar() -> Result<PermissionRules, MdmError> {
    let Some(path) = sidecar_path() else {
        return Ok(PermissionRules::default());
    };
    let body = read_or_empty(&path)?;
    if body.trim().is_empty() {
        return Ok(PermissionRules::default());
    }
    serde_json::from_str(&body).map_err(|source| MdmError::Json { path, source })
}

fn write_sidecar(rules: &PermissionRules) -> Result<(), MdmError> {
    let path = sidecar_path().ok_or(MdmError::Resolve("the bridge metadata directory"))?;
    let body = serde_json::to_string_pretty(rules).map_err(|source| MdmError::Json {
        path: path.clone(),
        source,
    })?;
    write_atomic(&path, &format!("{body}\n"))
}

fn read_json_object(
    path: &Path,
) -> Result<Option<serde_json::Map<String, serde_json::Value>>, MdmError> {
    let existing = read_or_empty(path)?;
    if existing.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&existing)
        .map(Some)
        .map_err(|source| MdmError::Json {
            path: path.to_path_buf(),
            source,
        })
}

fn splice_rules(
    root: &mut serde_json::Map<String, serde_json::Value>,
    previously_ours: &PermissionRules,
    rules: &PermissionRules,
) {
    match merged_permissions(root.get("permissions"), previously_ours, rules) {
        Some(permissions) => {
            root.insert("permissions".to_owned(), permissions);
        },
        None => {
            root.remove("permissions");
        },
    }
}

pub(crate) fn apply_permissions(rules: &PermissionRules) -> Result<Vec<String>, MdmError> {
    let previously = read_sidecar()?;
    let mut lines = Vec::new();
    let standalone =
        standalone_settings_path().ok_or(MdmError::Resolve("the user's config directory"))?;
    if let Some(mut root) = read_json_object(&standalone)? {
        splice_rules(&mut root, &previously, rules);
        write_atomic(&standalone, &render(root, &standalone)?)?;
        lines.push(format!(
            "wrote: {} (permissions, {} allow / {} deny)",
            standalone.display(),
            rules.allow.len(),
            rules.deny.len()
        ));
    }
    let settings = managed_settings_path().ok_or(MdmError::Resolve("the managed settings path"))?;
    // Why: only a settings file the bridge already configured (it holds our
    // apiKeyHelper) is ours to add rules to; a stranger's file is left alone.
    if let Some(mut root) = read_json_object(&settings)?
        && root.contains_key("apiKeyHelper")
    {
        splice_rules(&mut root, &previously, rules);
        write_atomic(&settings, &render(root, &settings)?)?;
        lines.push(format!(
            "wrote: {} (permissions, {} allow / {} deny)",
            settings.display(),
            rules.allow.len(),
            rules.deny.len()
        ));
    }
    if lines.is_empty() && !rules.is_empty() {
        return Err(MdmError::Resolve(
            "a Claude Code settings file to carry the permission rules",
        ));
    }
    write_sidecar(rules)?;
    Ok(lines)
}

pub(super) fn strip_owned_rules(
    root: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), MdmError> {
    let ours = read_sidecar()?;
    if !ours.is_empty() {
        splice_rules(root, &ours, &PermissionRules::default());
    }
    Ok(())
}

pub(super) fn remove_sidecar() -> Result<(), MdmError> {
    let Some(path) = sidecar_path() else {
        return Ok(());
    };
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_error("remove", &path)(e)),
    }
}
