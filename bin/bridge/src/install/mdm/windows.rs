//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

pub(super) fn refresh_managed_mcp_servers() -> Result<String, String> {
    let value = super::managed_mcp_servers_json().unwrap_or_else(|| "[]".to_owned());
    write_managed_mcp_servers_value(&value)
}

pub(super) fn write_managed_mcp_servers_value(value: &str) -> Result<String, String> {
    // HKCU: the per-user hive the non-elevated GUI can rewrite as the secret
    // rotates.
    let key = r"HKCU\SOFTWARE\Policies\Claude";
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
    // A stale HKLM copy outranks HKCU and breaks MCP auth; purge it (needs
    // elevation, may no-op).
    let stale = r"HKLM\SOFTWARE\Policies\Claude";
    _ = crate::winproc::reg_command()
        .args(["delete", stale, "/v", "managedMcpServers", "/f"])
        .status();
    Ok(format!(
        "{key} ← managedMcpServers (best-effort cleared {stale})"
    ))
}

pub(super) fn remove_policy() -> Result<bool, String> {
    let hkcu = crate::winproc::reg_command()
        .args(["delete", r"HKCU\SOFTWARE\Policies\Claude", "/f"])
        .status()
        .map(|s| s.success())
        .map_err(|e| format!("reg delete HKCU Policies\\Claude: {e}"))?;
    let hklm = crate::winproc::reg_command()
        .args([
            "delete",
            r"HKLM\SOFTWARE\Policies\Claude",
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
    for (name, kind, data) in values {
        let status = crate::winproc::reg_command()
            .args(["add", key, "/v", name, "/t", kind, "/d", &data, "/f"])
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
    if !elevated {
        summary.push(
            "warning: not running elevated — policy applied per-user (HKCU). Re-run from an \
             elevated shell to install machine-wide (HKLM)."
                .into(),
        );
    }
    if gateway.starts_with("http://") && !gateway.contains("://127.0.0.1") {
        summary.push(
            "warning: Bridge rejects http:// for non-127.0.0.1 hosts. Re-run --apply with http://127.0.0.1:<port> or switch to https://.".into(),
        );
    }
    summary.push("Fully quit Bridge (tray icon → Quit) and relaunch to pick up new policy.".into());
    Ok(summary)
}
