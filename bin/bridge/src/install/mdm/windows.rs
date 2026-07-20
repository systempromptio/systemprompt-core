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
    // Cowork >= 1.22209 enforces registry-policy hive precedence: when
    // `HKLM\SOFTWARE\Policies\Claude` exists, `HKCU\SOFTWARE\Policies\Claude`
    // is ignored ENTIRELY (Anthropic 3P docs, /cowork/3p/configuration). Our
    // `inference*` keys always live in HKLM, so `managedMcpServers` MUST be in
    // HKLM as well or Cowork loads zero managed servers (mcpServerCount:0, the
    // connector never appears). Older builds merged both hives, which is why
    // writing HKCU used to work — it no longer does.
    //
    // Writing HKLM requires elevation. If we are not elevated we cannot fix it:
    // clear any stale HKCU copy (so it is not mistaken for live config) and
    // fail loudly rather than silently write an HKCU value Cowork ignores.
    let hkcu = r"HKCU\SOFTWARE\Policies\Claude";
    let key = r"HKLM\SOFTWARE\Policies\Claude";
    if !crate::winproc::is_elevated() {
        // Best-effort clear the ignored HKCU copy so it can't be mistaken for
        // live config.
        _ = crate::winproc::reg_command()
            .args(["delete", hkcu, "/v", "managedMcpServers", "/f"])
            .status();
        // If an elevated run already provisioned managedMcpServers in HKLM we
        // cannot update it here, but we don't need to: the loopback bearer and
        // proxy port are stable, so the existing HKLM value stays valid. Treat
        // that as a no-op success instead of failing the whole host sync (which
        // would report PARTIAL on every non-elevated refresh). Only error when
        // HKLM has no value at all — i.e. it was never provisioned elevated.
        let already_in_hklm = crate::winproc::reg_command()
            .args(["query", key, "/v", "managedMcpServers"])
            .status()
            .is_ok_and(|s| s.success());
        if already_in_hklm {
            return Ok(format!(
                "{key} already holds managedMcpServers; skipping (elevation needed to \
                 rewrite, but the value is stable). Cleared ignored {hkcu} copy."
            ));
        }
        return Err(
            "managedMcpServers requires elevation: Cowork ignores HKCU when an HKLM policy \
             exists, so the managed server list must be written to HKLM\\SOFTWARE\\Policies\\Claude. \
             Re-run the Bridge from an elevated (Administrator) context."
                .to_owned(),
        );
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
    // Remove the now-ignored (and potentially stale) HKCU copy so it can never
    // be confused for the live value.
    _ = crate::winproc::reg_command()
        .args(["delete", hkcu, "/v", "managedMcpServers", "/f"])
        .status();
    Ok(format!("{key} ← managedMcpServers (cleared stale {hkcu})"))
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
    // Materialize the default workspace dir referenced by
    // `allowedWorkspaceFolders` (~/<brand workspace dir>) so Cowork's
    // pre-trusted folder chip resolves to an existing, writable directory
    // rather than prompting. Folder name is brand-specific, from the Brand.
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
