#![cfg(target_os = "windows")]

// Best-effort: errors return up but caller logs and ignores. Never abort
// the calling sync flow on registry write failure.
pub fn refresh_managed_mcp_servers() -> Result<String, String> {
    let value = super::managed_mcp_servers_json().unwrap_or_else(|| "[]".to_string());
    write_managed_mcp_servers_value(&value)
}

pub fn write_managed_mcp_servers_value(value: &str) -> Result<String, String> {
    let elevated = crate::winproc::is_elevated();
    let key = if elevated {
        r"HKLM\SOFTWARE\Policies\Claude"
    } else {
        r"HKCU\SOFTWARE\Policies\Claude"
    };
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
    // Single source of truth: a server left in the other hive (e.g. a stale
    // value an earlier non-elevated run wrote to HKCU) would be a second,
    // possibly out-of-date `managedMcpServers` definition Cowork might read.
    // Best-effort delete from the hive we did NOT write (clearing HKLM needs
    // elevation, so a non-elevated run may not succeed — that's acceptable).
    let other = if elevated {
        r"HKCU\SOFTWARE\Policies\Claude"
    } else {
        r"HKLM\SOFTWARE\Policies\Claude"
    };
    _ = crate::winproc::reg_command()
        .args(["delete", other, "/v", "managedMcpServers", "/f"])
        .status();
    Ok(format!("{key} ← managedMcpServers (cleared {other})"))
}

pub(super) fn apply(gateway: &str, pubkey: Option<&str>) -> Result<Vec<String>, String> {
    let elevated = crate::winproc::is_elevated();
    let key = if elevated {
        r"HKLM\SOFTWARE\Policies\Claude"
    } else {
        r"HKCU\SOFTWARE\Policies\Claude"
    };
    let values = super::windows_policy_values(gateway, pubkey);
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
