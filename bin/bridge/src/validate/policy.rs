//! The managed Claude Desktop policy and the workspace directory it names,
//! read back from where Cowork reads them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Report;

const REQUIRED: [(&str, &str); 4] = [
    (
        "allowedWorkspaceFolders",
        "missing — Cowork blocks on request_cowork_directory. Sync and approve the admin prompt.",
    ),
    (
        "inferenceProvider",
        "missing — Cowork is not routed through the gateway. Sync and approve the admin prompt.",
    ),
    (
        "inferenceGatewayBaseUrl",
        "missing — with inferenceProvider=gateway Cowork refuses to start any task. Sync and approve the admin prompt.",
    ),
    (
        "inferenceGatewayApiKey",
        "missing — Cowork cannot authenticate to the gateway. Sync and approve the admin prompt.",
    ),
];

const HARDENING: [(&str, &str); 6] = [
    ("disableEssentialTelemetry", "true"),
    ("disableNonessentialTelemetry", "true"),
    ("disableNonessentialServices", "false"),
    ("disableAutoUpdates", "true"),
    ("disableDeploymentModeChooser", "true"),
    ("isLocalDevMcpEnabled", "false"),
];

const BASH_GATES: [&str; 2] = ["disabledBuiltinTools", "builtinToolPolicy"];

// Why: none of this was validated before, so the health panel showed "all
// green" while the policy was half-written and Cowork was broken. A required
// value that is absent or unreadable fails the verdict.
pub(super) fn check_managed_policy(report: &mut Report) {
    let store = crate::config::store::managed_policy_store();
    for (key, remedy) in REQUIRED {
        match store.read_managed_policy(key) {
            Ok(Some(v)) if !v.trim().is_empty() => report.ok(&format!("policy {key}"), "set"),
            Ok(_) => report.fail(&format!("policy {key}"), remedy),
            Err(e) => report.fail(&format!("policy {key}"), &format!("unreadable: {e}")),
        }
    }
    match store.read_managed_policy("managedMcpServers") {
        Ok(Some(v)) if v.trim() == "[]" => {
            report.info("policy managedMcpServers", "none in manifest");
        },
        Ok(Some(_)) => report.ok("policy managedMcpServers", "set"),
        Ok(None) => report.warn("policy managedMcpServers", "not written — sync"),
        Err(e) => report.fail("policy managedMcpServers", &format!("unreadable: {e}")),
    }
    for (key, expected) in HARDENING {
        match store.read_managed_policy(key) {
            Ok(Some(v)) if v.trim() == expected => report.info(&format!("policy {key}"), &v),
            Ok(Some(v)) => report.fail(
                &format!("policy {key}"),
                &format!("is {v}, expected {expected} — sync to correct"),
            ),
            Ok(None) => report.warn(&format!("policy {key}"), "not set — sync to enforce"),
            Err(e) => report.fail(&format!("policy {key}"), &format!("unreadable: {e}")),
        }
    }
    // Why: these are the only policy keys that can deny Cowork's shell; a value
    // naming Bash breaks every skill that runs a command.
    for key in BASH_GATES {
        match store.read_managed_policy(key) {
            Ok(Some(v)) if v.contains("Bash") => report.fail(
                &format!("policy {key}"),
                &format!("names Bash ({v}) — Cowork shell denied; remove it"),
            ),
            Ok(Some(v)) => report.info(&format!("policy {key}"), &v),
            Ok(None) => report.ok(&format!("policy {key}"), "absent"),
            Err(e) => report.fail(&format!("policy {key}"), &format!("unreadable: {e}")),
        }
    }
    match store.read_managed_policy(crate::config::store::LEGACY_MANIFEST_PUBKEY_KEY) {
        Ok(Some(_)) => report.warn(
            "policy inferenceManifestPubkey",
            "stale copy in Claude's hive — Claude Desktop warns on every launch; sync moves it",
        ),
        Ok(None) => report.ok(
            "policy inferenceManifestPubkey",
            "absent from Claude's hive",
        ),
        Err(e) => report.fail(
            "policy inferenceManifestPubkey",
            &format!("unreadable: {e}"),
        ),
    }
    check_workspace_dir(report);
    check_claude_code_policy_dir(report);
    check_workspace_bundle(report);
}

// Why: this asks exactly the question `managed_mcp::clear_policy` asks, so the
// check and the remedy it names cannot disagree. Filenames alone used to decide
// it, which permanently failed a machine holding an empty
// `managed-settings.json` — that file shadows nothing, sync correctly leaves
// it, and the named remedy could never clear it.
fn check_claude_code_policy_dir(report: &mut Report) {
    use crate::claude_policy::{MANAGED_MCP_FILE, MANAGED_SETTINGS_FILE};

    let dir = crate::config::paths::claude_code_policy_dir();
    if !dir.exists() {
        report.ok("claude code policy dir", "absent");
        return;
    }
    let mut offenders: Vec<&str> = Vec::new();
    // Why: exclusive mode is switched on by this file existing at all.
    if dir.join(MANAGED_MCP_FILE).exists() {
        offenders.push(MANAGED_MCP_FILE);
    }
    let settings = dir.join(MANAGED_SETTINGS_FILE);
    match crate::claude_policy::stripped_settings(&settings) {
        Ok(Some(_)) => offenders.push(MANAGED_SETTINGS_FILE),
        Ok(None) => {},
        Err(e) => report.warn(
            "claude code policy dir",
            &format!("{} unreadable: {e}", settings.display()),
        ),
    }
    if offenders.is_empty() {
        report.ok("claude code policy dir", "no MCP policy files");
    } else {
        report.fail(
            "claude code policy dir",
            &format!(
                "{} holds {} — Cowork tools shadowed; sync removes them",
                dir.display(),
                offenders.join(", ")
            ),
        );
    }
}

fn check_workspace_bundle(report: &mut Report) {
    let Some(dir) = crate::config::paths::workspace_artifacts_dir() else {
        return;
    };
    if dir.join("manifest.json").is_file() {
        report.ok("workspace dashboards", &dir.display().to_string());
    } else {
        report.warn(
            "workspace dashboards",
            "not staged — sync stages the dashboard bundle for the setup skills",
        );
    }
}

// Why: Cowork prompts for the workspace even when the policy names it unless
// the directory exists on disk.
fn check_workspace_dir(report: &mut Report) {
    let workspace = crate::brand::brand().workspace_dir_name;
    if workspace.is_empty() {
        return;
    }
    let Some(ws) = crate::config::paths::workspace_dir() else {
        report.fail("workspace dir", "no home directory to resolve it under");
        return;
    };
    if ws.is_dir() {
        report.ok("workspace dir", &ws.display().to_string());
    } else {
        report.fail(
            "workspace dir",
            &format!(
                "{} missing — Cowork will prompt for it. Sync.",
                ws.display()
            ),
        );
    }
}
