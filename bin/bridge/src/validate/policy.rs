//! The managed Claude Desktop policy and the workspace directory it names,
//! read back from where Cowork reads them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Report;

const REQUIRED: [(&str, &str); 3] = [
    (
        "allowedWorkspaceFolders",
        "missing — Cowork blocks on request_cowork_directory. Sync and approve the admin prompt.",
    ),
    (
        "inferenceGatewayBaseUrl",
        "missing — Cowork has no model routing. Re-apply the Claude Desktop host profile.",
    ),
    (
        "inferenceGatewayApiKey",
        "missing — Cowork cannot authenticate to the gateway. Re-apply the Claude Desktop host profile.",
    ),
];

const HARDENING: [&str; 6] = [
    "disableEssentialTelemetry",
    "disableNonessentialTelemetry",
    "disableNonessentialServices",
    "disableAutoUpdates",
    "disableDeploymentModeChooser",
    "isLocalDevMcpEnabled",
];

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
    for key in HARDENING {
        match store.read_managed_policy(key) {
            Ok(Some(v)) => report.info(&format!("policy {key}"), &v),
            Ok(None) => report.warn(&format!("policy {key}"), "not set — sync to enforce"),
            Err(e) => report.fail(&format!("policy {key}"), &format!("unreadable: {e}")),
        }
    }
    check_workspace_dir(report);
}

// Why: Cowork prompts for the workspace even when the policy names it unless
// the directory exists on disk.
fn check_workspace_dir(report: &mut Report) {
    let workspace = crate::brand::brand().workspace_dir_name;
    if workspace.is_empty() {
        return;
    }
    let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) else {
        report.fail("workspace dir", "no home directory to resolve it under");
        return;
    };
    let ws = std::path::Path::new(&home).join(workspace);
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
