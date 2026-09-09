//! Removing the managed Claude Desktop profile on uninstall, whole rather
//! than one value at a time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(target_os = "macos")]
pub(super) fn remove() -> super::ManagedProfileOutcome {
    match super::mdm::macos::remove_profile() {
        Ok(true) => super::ManagedProfileOutcome::Removed(super::mdm::macos::PAYLOAD_IDENTIFIER),
        Ok(false) => {
            super::ManagedProfileOutcome::NotInstalled(super::mdm::macos::PAYLOAD_IDENTIFIER)
        },
        Err(e) => {
            let msg = format!("profile remove failed: {e}");
            crate::stdio::diag(&msg);
            super::ManagedProfileOutcome::RemoveFailed(msg)
        },
    }
}

#[cfg(target_os = "windows")]
pub(super) fn remove() -> super::ManagedProfileOutcome {
    let removed = match super::mdm::remove_windows_policy() {
        Ok(removed) => removed,
        Err(e) => {
            crate::stdio::diag(&e.to_string());
            return super::ManagedProfileOutcome::RemoveFailed(e.to_string());
        },
    };
    // Why: the machine policy carries the inference gateway and secret as
    // well as the connector list; a purge that left those behind kept Claude
    // Desktop pointed at a proxy that no longer exists.
    match remove_machine_claude_policy() {
        Ok(true) => {
            super::ManagedProfileOutcome::Removed("HKLM Policies\\Claude (+ any HKCU copy)")
        },
        Ok(false) if removed => super::ManagedProfileOutcome::Removed(
            "HKLM Policies\\Claude managedMcpServers (+ any HKCU copy)",
        ),
        Ok(false) => super::ManagedProfileOutcome::NotInstalled("Windows Policies\\Claude"),
        Err(e) => {
            crate::stdio::diag(&e.to_string());
            super::ManagedProfileOutcome::RemoveFailed(e.to_string())
        },
    }
}

#[cfg(target_os = "windows")]
fn remove_machine_claude_policy() -> std::io::Result<bool> {
    use crate::config::store::{PolicyHive, PolicyTarget, managed_policy_store};
    let keys = crate::cowork_compat::POLICY_KEYS;
    let store = managed_policy_store();
    if !store
        .policy_key_exists(PolicyHive::Machine, PolicyTarget::Claude)
        .map_err(std::io::Error::other)?
    {
        return Ok(false);
    }
    let present = store
        .read_policy_document(PolicyHive::Machine, PolicyTarget::Claude, keys)
        .map_err(std::io::Error::other)?;
    if present.is_empty() {
        return Ok(false);
    }
    if crate::winproc::is_elevated() {
        let removed = crate::config::store::verified::remove_values(
            store.as_ref(),
            PolicyHive::Machine,
            PolicyTarget::Claude,
            keys,
        )
        .map_err(std::io::Error::other)?;
        return Ok(removed > 0);
    }
    let stage_dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    std::fs::create_dir_all(&stage_dir)?;
    let job = super::elevated_job::ElevatedJob {
        reg_path: None,
        org_plugins: None,
        clear_values: keys.iter().map(|k| (*k).to_owned()).collect(),
        bridge_values: Vec::new(),
        managed_files: Vec::new(),
        remove_files: Vec::new(),
    };
    super::elevated_job::elevate_and_run(&stage_dir, &job)?.require(
        "clear_policy",
        std::path::Path::new(crate::cowork_compat::HKLM_POLICY_KEY),
    )?;
    Ok(true)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(super) fn remove() -> super::ManagedProfileOutcome {
    let lines = match super::mdm::linux::remove() {
        Ok(lines) => lines,
        Err(e) => return super::ManagedProfileOutcome::RemoveFailed(e.to_string()),
    };
    if lines.is_empty() {
        return super::ManagedProfileOutcome::NotInstalled("Linux env configuration");
    }
    for line in &lines {
        tracing::info!(target: "bridge::install", detail = %line, "linux env cleanup");
    }
    super::ManagedProfileOutcome::Removed("Linux env configuration (env.sh + ~/.profile block)")
}
