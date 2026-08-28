//! Windows MDM (registry policy) deployment snippet rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

pub(super) fn refresh_managed_mcp_servers() -> Result<String, String> {
    let value = super::managed_mcp_servers_json().unwrap_or_else(|| "[]".to_owned());
    write_managed_mcp_servers_value(&value)
}

pub(super) fn write_managed_mcp_servers_value(value: &str) -> Result<String, String> {
    // Why: HKLM writes need elevation — hence the drift-only UAC.
    let hkcu = crate::cowork_compat::HKCU_POLICY_KEY;
    let key = crate::cowork_compat::HKLM_POLICY_KEY;
    if !crate::winproc::is_elevated() {
        _ = crate::winproc::reg_command()
            .args(["delete", hkcu, "/v", "managedMcpServers", "/f"])
            .status();
        if current_value().as_deref() == Some(value) {
            return Ok(format!(
                "{key} already holds this managedMcpServers value; skipping (cleared \
                 ignored {hkcu} copy)."
            ));
        }
        return elevated_write(value).map(|()| format!("{key} ← managedMcpServers (elevated)"));
    }
    let status = crate::winproc::reg_command()
        .args([
            "add",
            key,
            "/v",
            "managedMcpServers",
            "/t",
            "REG_SZ",
            "/d",
            value,
            "/f",
        ])
        .status()
        .map_err(|e| format!("reg add managedMcpServers: {e}"))?;
    if !status.success() {
        return Err(format!(
            "reg add managedMcpServers exited with {}",
            status.code().unwrap_or(-1)
        ));
    }
    _ = crate::winproc::reg_command()
        .args(["delete", hkcu, "/v", "managedMcpServers", "/f"])
        .status();
    Ok(format!("{key} ← managedMcpServers (cleared stale {hkcu})"))
}

fn current_value() -> Option<String> {
    match crate::config::store::managed_policy_store().read_managed_policy("managedMcpServers") {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                target: "bridge::install::mdm",
                error = %e,
                "could not read the current managedMcpServers policy value"
            );
            None
        },
    }
}

fn elevated_write(value: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create staging dir: {e}"))?;
    let path = dir.join("managed-mcp-servers.reg");
    let body = crate::integration::claude_desktop::reg_profile::render_reg_values(
        true,
        &[("managedMcpServers", value.to_owned())],
    );
    std::fs::write(&path, body).map_err(|e| format!("stage managedMcpServers profile: {e}"))?;
    tracing::info!(
        target: "bridge::install::mdm",
        path = %path.display(),
        "managed MCP server list drifted; requesting elevation to update HKLM policy"
    );
    let job = crate::integration::claude_desktop::elevate::ElevatedJob {
        clear_values: Vec::new(),
        reg_path: Some(path.to_string_lossy().into_owned()),
        org_plugins:
            crate::integration::claude_desktop::elevate::ElevatedJob::org_plugins_for_current_user(),
    };
    crate::integration::claude_desktop::elevate::elevate_and_run(&dir, &job).map_err(|e| {
        format!(
            "the MCP connector list could not be updated: {e}. Re-run the Bridge as \
                 Administrator to apply it."
        )
    })
}

pub(super) fn remove_policy() -> Result<bool, String> {
    let hkcu = crate::winproc::reg_command()
        .args(["delete", crate::cowork_compat::HKCU_POLICY_KEY, "/f"])
        .status()
        .map(|s| s.success())
        .map_err(|e| format!("reg delete HKCU Policies\\Claude: {e}"))?;
    let hklm = crate::winproc::reg_command()
        .args([
            "delete",
            crate::cowork_compat::HKLM_POLICY_KEY,
            "/v",
            "managedMcpServers",
            "/f",
        ])
        .status()
        .is_ok_and(|s| s.success());
    Ok(hkcu || hklm)
}

pub(super) fn apply(gateway: &str, pubkey: Option<&str>) -> Result<Vec<String>, String> {
    let elevated = crate::winproc::is_elevated();
    let key = if elevated {
        r"HKLM\SOFTWARE\Policies\Claude"
    } else {
        r"HKCU\SOFTWARE\Policies\Claude"
    };
    let org_uuid = crate::config::load().deployment_organization_uuid;
    let values = super::windows_policy_values(gateway, pubkey, org_uuid.as_deref());
    let mut summary = Vec::with_capacity(values.len() + 2);
    summary.push(format!("registry key: {key}"));
    for (name, kind, data) in &values {
        let status = crate::winproc::reg_command()
            .args(["add", key, "/v", name, "/t", kind, "/d", data, "/f"])
            .status()
            .map_err(|e| format!("reg add {name}: {e}"))?;
        if !status.success() {
            return Err(format!(
                "reg add {name} exited with {}",
                status.code().unwrap_or(-1)
            ));
        }
        summary.push(format!("wrote {name} ({kind})"));
    }
    // Why: Cowork prompts instead of pre-trusting unless the directory named by
    // `allowedWorkspaceFolders` already exists on disk.
    let workspace = crate::brand::brand().workspace_dir_name;
    if !workspace.is_empty()
        && let Some(home) = std::env::var_os("USERPROFILE")
    {
        let ws = std::path::Path::new(&home).join(workspace);
        match std::fs::create_dir_all(&ws) {
            Ok(()) => summary.push(format!("ensured workspace dir {}", ws.display())),
            Err(e) => {
                summary.push(format!(
                    "warning: could not create workspace dir {}: {e}",
                    ws.display()
                ));
            },
        }
    }
    let org_job =
        crate::integration::claude_desktop::elevate::ElevatedJob::org_plugins_for_current_user();
    if elevated {
        if let Some(org) = org_job {
            crate::integration::claude_desktop::elevate::provision_org_plugins(
                &org.path,
                &org.grant_user,
            )
            .map_err(|e| format!("org-plugins provisioning failed: {e}"))?;
            summary.push(format!(
                "provisioned {} with a Modify grant for {}",
                org.path.display(),
                org.grant_user
            ));
        }
    } else {
        match stage_elevated_apply(&values, org_job) {
            Ok(msg) => summary.push(msg),
            Err(e) => summary.push(format!("warning: {e}")),
        }
    }
    if gateway.starts_with("http://") && !gateway.contains("://127.0.0.1") {
        summary.push(
            "warning: Bridge rejects http:// for non-127.0.0.1 hosts. Re-run --apply with http://127.0.0.1:<port> or switch to https://.".into(),
        );
    }
    summary.push("Fully quit Bridge (tray icon → Quit) and relaunch to pick up new policy.".into());
    Ok(summary)
}

// Why: the error shown when Cowork sync cannot write org-plugins tells the
// user to re-run `install --apply` and approve ONE administrator prompt — this
// is the step that makes that promise true: one UAC pass writes the HKLM
// policy and grants the invoking user Modify on org-plugins.
fn stage_elevated_apply(
    values: &[(&'static str, &'static str, String)],
    org_plugins: Option<crate::integration::claude_desktop::elevate::OrgPluginsJob>,
) -> Result<String, String> {
    let dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create staging dir: {e}"))?;
    let entries: Vec<(&str, String)> = values.iter().map(|(n, _, d)| (*n, d.clone())).collect();
    let body = crate::integration::claude_desktop::reg_profile::render_reg_values(true, &entries);
    let path = dir.join("bridge-policy-apply.reg");
    std::fs::write(&path, body).map_err(|e| format!("stage policy profile: {e}"))?;
    let job = crate::integration::claude_desktop::elevate::ElevatedJob {
        clear_values: Vec::new(),
        reg_path: Some(path.to_string_lossy().into_owned()),
        org_plugins,
    };
    crate::integration::claude_desktop::elevate::elevate_and_run(&dir, &job)
        .map(|()| {
            "elevated step complete: HKLM policy written and org-plugins provisioned".to_owned()
        })
        .map_err(|e| {
            format!(
                "elevated step did not complete ({e}); policy applied per-user (HKCU) and \
                 org-plugins was not provisioned — Cowork sync may fail"
            )
        })
}
