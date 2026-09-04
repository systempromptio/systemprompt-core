//! Windows MDM (registry policy) deployment snippet rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

use super::error::MdmError;
use super::windows_policy;

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
        bridge_values: Vec::new(),
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
    let pubkey = crate::config::pinned_pubkey();
    let values = policy_values(inputs, &inputs.loopback.origin())?;
    let bridge = super::bridge_policy_values(pubkey.as_ref().map(crate::ids::PinnedPubKey::as_str));
    let plan = windows_policy::WritePlan::new(&values, &bridge);
    if !plan.drifted() {
        return Ok("managed policy already in step; nothing to write".into());
    }
    if crate::winproc::is_elevated() {
        plan.write_in_process()?;
        return Ok(format!(
            "{} ← full managed policy re-asserted (elevated)",
            crate::cowork_compat::HKLM_POLICY_KEY
        ));
    }
    let org_job = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user();
    plan.stage_elevated(org_job)
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

// Why: `apply` and `enforce_managed_policy` both write the whole policy, and
// assembling it twice is how the two drifted apart before. Both take it from
// here, and `policy::claude_desktop_policy` is the only place the key set is
// decided for either platform.
fn policy_values(
    inputs: &super::MdmPayloadInputs<'_>,
    base_url: &str,
) -> Result<Vec<(&'static str, &'static str, String)>, MdmError> {
    let secret = inputs.loopback.secret().map_err(|e| {
        MdmError::Windows(format!(
            "loopback secret unavailable ({e}); the gateway policy block was not written. Start \
             the Bridge proxy, then sync again."
        ))
    })?;
    let servers = super::policy::mcp_entries(inputs.loopback, inputs.registry).map_err(|e| {
        MdmError::Windows(format!("the MCP connector list could not be built: {e}"))
    })?;
    let existing_models = crate::config::store::managed_policy_store()
        .read_managed_policy("inferenceModels")
        .ok()
        .flatten();
    let policy = super::policy::claude_desktop_policy(&super::policy::PolicyInputs {
        base_url,
        api_key: secret.as_str(),
        models: existing_models,
        headers: &std::collections::BTreeMap::new(),
        egress_allowed_hosts: inputs.egress_allowed_hosts,
        org_uuid: crate::config::load()
            .deployment_organization_uuid
            .as_deref(),
        mcp_servers: &servers,
    });
    Ok(super::policy::reg_values(&policy))
}

pub(super) fn apply(
    inputs: &super::MdmPayloadInputs<'_>,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<Vec<String>, MdmError> {
    let elevated = crate::winproc::is_elevated();
    let key = crate::cowork_compat::HKLM_POLICY_KEY;
    let values = policy_values(inputs, gateway)?;
    let bridge = super::bridge_policy_values(pubkey);
    let plan = windows_policy::WritePlan::new(&values, &bridge);
    let mut summary = Vec::with_capacity(values.len() + bridge.len() + 4);
    summary.push(format!("registry key: {key}"));
    summary.extend(ensure_workspace_dir());
    let org_job = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user();
    // Why: `SOFTWARE\Policies` is machine policy and Cowork ignores HKCU once
    // the HKLM key exists, so an unelevated run stages ONE administrator pass
    // rather than writing a per-user copy that would never be read.
    if elevated {
        plan.write_in_process()?;
        for (name, kind, _) in values.iter().chain(&bridge) {
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
        summary.push(plan.stage_elevated(org_job)?);
    }
    if gateway.starts_with("http://") && !gateway.contains("://127.0.0.1") {
        summary.push(
            "warning: Bridge rejects http:// for non-127.0.0.1 hosts. Re-run --apply with http://127.0.0.1:<port> or switch to https://.".into(),
        );
    }
    summary.push("Fully quit Bridge (tray icon → Quit) and relaunch to pick up new policy.".into());
    Ok(summary)
}
