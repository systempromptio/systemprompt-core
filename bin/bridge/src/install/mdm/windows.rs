//! Windows MDM (registry policy) deployment snippet rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

use super::error::MdmError;

pub(super) fn write_managed_mcp_servers_value(value: &str) -> Result<String, MdmError> {
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
        .map_err(|e| MdmError::Windows(format!("reg add managedMcpServers: {e}")))?;
    if !status.success() {
        return Err(MdmError::Windows(format!(
            "reg add managedMcpServers exited with {}",
            status.code().unwrap_or(-1)
        )));
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

fn elevated_write(value: &str) -> Result<(), MdmError> {
    let dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    std::fs::create_dir_all(&dir).map_err(|source| MdmError::Io {
        action: "create staging dir",
        path: dir.clone(),
        source,
    })?;
    let path = dir.join("managed-mcp-servers.reg");
    let body = crate::install::reg_values::render_reg_values(
        true,
        &[("managedMcpServers", value.to_owned())],
    );
    std::fs::write(&path, body).map_err(|source| MdmError::Io {
        action: "stage managedMcpServers profile",
        path: path.clone(),
        source,
    })?;
    tracing::info!(
        target: "bridge::install::mdm",
        path = %path.display(),
        "managed MCP server list drifted; requesting elevation to update HKLM policy"
    );
    let job = crate::install::elevated_job::ElevatedJob {
        clear_values: Vec::new(),
        managed_files: Vec::new(),
        remove_files: Vec::new(),
        reg_path: Some(path.to_string_lossy().into_owned()),
        org_plugins: crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user(),
    };
    crate::install::elevated_job::elevate_and_run(&dir, &job).map_err(|e| {
        MdmError::Windows(format!(
            "the MCP connector list could not be updated: {e}. Re-run the Bridge as \
                 Administrator to apply it."
        ))
    })
}

// Why: sync used to keep only `managedMcpServers` in step, so a policy key
// missing `allowedWorkspaceFolders` (a fresh machine, a wiped policy) left
// Cowork prompting on `request_cowork_directory` with telemetry and
// auto-update back on. Every managed value is re-asserted here, with ONE
// administrator prompt only when a value has actually drifted.
pub(super) fn enforce_managed_policy(
    inputs: &super::MdmPayloadInputs<'_>,
) -> Result<String, MdmError> {
    ensure_workspace_dir();
    let org_uuid = crate::config::load().deployment_organization_uuid;
    let pubkey = crate::config::pinned_pubkey();
    let mut values = super::windows_policy_values(
        pubkey.as_ref().map(crate::ids::PinnedPubKey::as_str),
        org_uuid.as_deref(),
        inputs.egress_allowed_hosts,
    );
    let mcp = super::managed_mcp_servers_json(inputs).unwrap_or_else(|| "[]".to_owned());
    values.push(("managedMcpServers", "REG_SZ", mcp));

    if !policy_drifted(&values) {
        return Ok("managed policy already in step; nothing to write".into());
    }
    if crate::winproc::is_elevated() {
        write_values_in_process(&values)?;
        return Ok(format!(
            "{} ← full managed policy re-asserted (elevated)",
            crate::cowork_compat::HKLM_POLICY_KEY
        ));
    }
    let org_job = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user();
    stage_elevated_apply(&values, org_job)
}

fn write_values_in_process(
    values: &[(&'static str, &'static str, String)],
) -> Result<(), MdmError> {
    let key = crate::cowork_compat::HKLM_POLICY_KEY;
    for (name, kind, data) in values {
        let status = crate::winproc::reg_command()
            .args(["add", key, "/v", name, "/t", kind, "/d", data, "/f"])
            .status()
            .map_err(|e| MdmError::Windows(format!("reg add {name}: {e}")))?;
        if !status.success() {
            return Err(MdmError::Windows(format!(
                "reg add {name} exited with {}",
                status.code().unwrap_or(-1)
            )));
        }
    }
    Ok(())
}

// Why: a read error counts as drift — an unknown on-disk state must never be
// mistaken for an up-to-date one.
fn policy_drifted(values: &[(&'static str, &'static str, String)]) -> bool {
    let store = crate::config::store::managed_policy_store();
    values.iter().any(|(name, _, data)| {
        !matches!(store.read_managed_policy(name), Ok(Some(current)) if &current == data)
    })
}

// Why: Cowork prompts instead of pre-trusting unless the directory named by
// `allowedWorkspaceFolders` already exists on disk.
fn ensure_workspace_dir() -> Option<String> {
    let workspace = crate::brand::brand().workspace_dir_name;
    if workspace.is_empty() {
        return None;
    }
    let home = std::env::var_os("USERPROFILE")?;
    let ws = std::path::Path::new(&home).join(workspace);
    match std::fs::create_dir_all(&ws) {
        Ok(()) => Some(format!("ensured workspace dir {}", ws.display())),
        Err(e) => {
            tracing::warn!(
                target: "bridge::install::mdm",
                error = %e,
                path = %ws.display(),
                "could not create pre-trusted workspace dir"
            );
            Some(format!(
                "warning: could not create workspace dir {}: {e}",
                ws.display()
            ))
        },
    }
}

pub(super) fn remove_policy() -> Result<bool, MdmError> {
    let hkcu = crate::winproc::reg_command()
        .args(["delete", crate::cowork_compat::HKCU_POLICY_KEY, "/f"])
        .status()
        .map(|s| s.success())
        .map_err(|e| MdmError::Windows(format!("reg delete HKCU Policies\\Claude: {e}")))?;
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

pub(super) fn apply(
    inputs: &super::MdmPayloadInputs<'_>,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<Vec<String>, MdmError> {
    let elevated = crate::winproc::is_elevated();
    let key = crate::cowork_compat::HKLM_POLICY_KEY;
    let org_uuid = crate::config::load().deployment_organization_uuid;
    let mut values =
        super::windows_policy_values(pubkey, org_uuid.as_deref(), inputs.egress_allowed_hosts);
    let mcp = super::managed_mcp_servers_json(inputs).unwrap_or_else(|| "[]".to_owned());
    values.push(("managedMcpServers", "REG_SZ", mcp));
    let mut summary = Vec::with_capacity(values.len() + 4);
    summary.push(format!("registry key: {key}"));
    summary.extend(ensure_workspace_dir());
    let org_job = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user();
    // Why: `SOFTWARE\Policies` is machine policy and Cowork ignores HKCU once
    // the HKLM key exists, so an unelevated run stages ONE administrator pass
    // rather than writing a per-user copy that would never be read.
    if elevated {
        write_values_in_process(&values)?;
        for (name, kind, _) in &values {
            summary.push(format!("wrote {name} ({kind})"));
        }
        if let Some(org) = org_job {
            crate::install::elevated_job::provision_org_plugins(&org.path, &org.grant_user)
                .map_err(|e| MdmError::Windows(format!("org-plugins provisioning failed: {e}")))?;
            summary.push(format!(
                "provisioned {} with a Modify grant for {}",
                org.path.display(),
                org.grant_user
            ));
        }
    } else {
        summary.push(stage_elevated_apply(&values, org_job)?);
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
    org_plugins: Option<crate::install::elevated_job::OrgPluginsJob>,
) -> Result<String, MdmError> {
    let dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    std::fs::create_dir_all(&dir).map_err(|source| MdmError::Io {
        action: "create staging dir",
        path: dir.clone(),
        source,
    })?;
    let entries: Vec<(&str, String)> = values.iter().map(|(n, _, d)| (*n, d.clone())).collect();
    let body = crate::install::reg_values::render_reg_values(true, &entries);
    let path = dir.join("bridge-policy-apply.reg");
    std::fs::write(&path, body).map_err(|source| MdmError::Io {
        action: "stage policy profile",
        path: path.clone(),
        source,
    })?;
    let job = crate::install::elevated_job::ElevatedJob {
        clear_values: Vec::new(),
        managed_files: Vec::new(),
        remove_files: Vec::new(),
        reg_path: Some(path.to_string_lossy().into_owned()),
        org_plugins,
    };
    crate::install::elevated_job::elevate_and_run(&dir, &job)
        .map(|()| {
            "elevated step complete: HKLM policy written and org-plugins provisioned".to_owned()
        })
        .map_err(|e| {
            MdmError::Windows(format!(
                "elevated step did not complete ({e}); the machine policy was not written and \
                 org-plugins was not provisioned — Cowork stays unmanaged"
            ))
        })
}
