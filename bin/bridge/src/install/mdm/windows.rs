#![cfg(target_os = "windows")]

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
            "warning: Cowork rejects http:// for non-127.0.0.1 hosts. Re-run --apply with http://127.0.0.1:<port> or switch to https://.".into(),
        );
    }
    summary.push("Fully quit Cowork (tray icon → Quit) and relaunch to pick up new policy.".into());
    Ok(summary)
}
