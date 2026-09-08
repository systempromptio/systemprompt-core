//! Windows MDM (registry policy) deployment snippet rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

use super::error::MdmError;
use super::windows_policy;
use crate::config::store::{PolicyHive, PolicyWrite, hive_for};

pub(super) fn write_managed_mcp_servers_value(value: &str) -> Result<String, MdmError> {
    let elevated = crate::winproc::is_elevated();
    let hive = hive_for(elevated);
    let key = policy_key(hive);
    if current_value(hive).as_deref() == Some(value) {
        return Ok(format!(
            "{key} already holds this managedMcpServers value; nothing to write"
        ));
    }
    let entries = [("managedMcpServers".to_owned(), value.to_owned())];
    let outcome = crate::config::store::write_managed_claude_policy(elevated, &entries)
        .map_err(windows_policy::policy_err)?;
    if elevated {
        clear_stale_user_value("managedMcpServers");
    }
    Ok(match outcome {
        PolicyWrite::Written(hive) => format!("{} ← managedMcpServers", policy_key(hive)),
        PolicyWrite::SatisfiedByMachine => format!(
            "{} already holds this managedMcpServers value; the per-user copy was not written",
            crate::cowork_compat::HKLM_POLICY_KEY
        ),
    })
}

const fn policy_key(hive: PolicyHive) -> &'static str {
    match hive {
        PolicyHive::Machine => crate::cowork_compat::HKLM_POLICY_KEY,
        PolicyHive::User => crate::cowork_compat::HKCU_POLICY_KEY,
    }
}

fn current_value(hive: PolicyHive) -> Option<String> {
    match crate::config::store::managed_policy_store()
        .read_policy_document(hive, &["managedMcpServers"])
    {
        Ok(doc) => doc
            .get("managedMcpServers")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        Err(e) => {
            tracing::warn!(
                target: "bridge::install::mdm",
                hive = hive.label(),
                error = %e,
                "could not read the current managedMcpServers policy value"
            );
            None
        },
    }
}

// Why: an elevated write to HKLM leaves any older per-user copy behind, and a
// stale HKCU value is what a later unelevated run would otherwise read back.
fn clear_stale_user_value(name: &str) {
    match crate::config::store::clear_managed_claude_policy(false, &[name]) {
        Ok(0) => {},
        Ok(n) => tracing::info!(
            target: "bridge::install::mdm",
            name,
            removed = n,
            "cleared stale HKCU policy value"
        ),
        Err(e) => tracing::warn!(
            target: "bridge::install::mdm",
            name,
            error = %e,
            "stale HKCU policy value could not be cleared"
        ),
    }
}

/// The one thing an ordinary process cannot do: `Program
/// Files\Claude\org-plugins` is admin-write-only. Say so once, without blocking
/// the policy write.
fn org_plugins_note() -> Option<String> {
    let org = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user()?;
    if org.path.is_dir() {
        return None;
    }
    Some(format!(
        "note: {} is not provisioned; run `{} install --apply` as Administrator once so Cowork \
         can read org-plugins",
        org.path.display(),
        crate::brand::brand().binary_name
    ))
}

pub(super) fn enforce_managed_policy(
    inputs: &super::MdmPayloadInputs<'_>,
) -> Result<String, MdmError> {
    ensure_workspace_dir();
    let pubkey = crate::config::pinned_pubkey();
    let values = policy_values(inputs, &inputs.loopback.origin())?;
    let bridge = super::bridge_policy_values(pubkey.as_ref().map(crate::ids::PinnedPubKey::as_str));
    let elevated = crate::winproc::is_elevated();
    let plan = windows_policy::WritePlan::new(&values, &bridge, elevated);
    if !plan.drifted() {
        return Ok(format!(
            "{} managed policy already in step; nothing to write",
            plan.hive().label()
        ));
    }
    let mut line = match plan.write()? {
        PolicyWrite::Written(hive) => {
            format!("{} ← full managed policy re-asserted", policy_key(hive))
        },
        PolicyWrite::SatisfiedByMachine => format!(
            "{} already holds the full managed policy; per-user copy not written",
            crate::cowork_compat::HKLM_POLICY_KEY
        ),
    };
    if !elevated && let Some(note) = org_plugins_note() {
        tracing::warn!(target: "bridge::install::mdm", "{note}");
        line.push_str("; ");
        line.push_str(&note);
    }
    Ok(line)
}

// Why: Cowork pre-trusts allowedWorkspaceFolders only when the directory
// already exists.
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
    let store = crate::config::store::managed_policy_store();
    let hkcu = store
        .delete_policy_key(PolicyHive::User)
        .map_err(windows_policy::policy_err)?;
    let hklm = match store.delete_policy_values(PolicyHive::Machine, &["managedMcpServers"]) {
        Ok(n) => n > 0,
        Err(e) => {
            tracing::warn!(
                target: "bridge::install::mdm",
                error = %e,
                "HKLM managedMcpServers value could not be removed"
            );
            false
        },
    };
    Ok(hkcu || hklm)
}

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
    let values = policy_values(inputs, gateway)?;
    let bridge = super::bridge_policy_values(pubkey);
    let plan = windows_policy::WritePlan::new(&values, &bridge, elevated);
    let key = policy_key(plan.hive());
    let mut summary = Vec::with_capacity(values.len() + bridge.len() + 4);
    summary.push(format!("registry key: {key}"));
    summary.extend(ensure_workspace_dir());
    match plan.write()? {
        PolicyWrite::Written(_) => {
            for (name, kind, _) in values.iter().chain(&bridge) {
                summary.push(format!("wrote {name} ({kind}) — verified by read-back"));
            }
        },
        PolicyWrite::SatisfiedByMachine => summary.push(format!(
            "{} already holds these values; per-user copy not written",
            crate::cowork_compat::HKLM_POLICY_KEY
        )),
    }
    if elevated {
        if let Some(org) = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user()
        {
            crate::install::elevated_job::provision_org_plugins(&org.path, &org.grant_user)
                .map_err(|e| MdmError::Windows(format!("org-plugins provisioning failed: {e}")))?;
            summary.push(format!(
                "provisioned {} with a Modify grant for {}",
                org.path.display(),
                org.grant_user
            ));
        }
    } else {
        summary.extend(org_plugins_note());
    }
    if gateway.starts_with("http://") && !gateway.contains("://127.0.0.1") {
        summary.push(
            "warning: Bridge rejects http:// for non-127.0.0.1 hosts. Re-run --apply with http://127.0.0.1:<port> or switch to https://.".into(),
        );
    }
    summary.push("Fully quit Bridge (tray icon → Quit) and relaunch to pick up new policy.".into());
    Ok(summary)
}
