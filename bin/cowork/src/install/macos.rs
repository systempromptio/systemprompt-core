#[cfg(target_os = "macos")]
use super::xml;
use std::path::Path;

#[cfg(target_os = "macos")]
pub(crate) const PAYLOAD_IDENTIFIER: &str = "io.systemprompt.cowork.mdm";
#[cfg(target_os = "macos")]
const INNER_PAYLOAD_IDENTIFIER: &str = "io.systemprompt.cowork.mdm.inference";
#[cfg(target_os = "macos")]
pub(crate) const MANAGED_PREFS_PATH: &str =
    "/Library/Managed Preferences/com.anthropic.claudefordesktop.plist";

#[cfg(target_os = "macos")]
const PREFS_PLIST_TMPL: &str = include_str!("templates/prefs.plist.tmpl");
#[cfg(target_os = "macos")]
const PREFS_PUBKEY_LINE_TMPL: &str = include_str!("templates/prefs_pubkey_line.tmpl");
#[cfg(target_os = "macos")]
const MOBILECONFIG_TMPL: &str = include_str!("templates/mobileconfig.tmpl");
#[cfg(target_os = "macos")]
const MOBILECONFIG_PUBKEY_LINE_TMPL: &str =
    include_str!("templates/mobileconfig_pubkey_line.tmpl");

#[cfg(target_os = "macos")]
pub fn build_prefs_plist(binary: &Path, gateway: &str, pubkey: Option<&str>) -> String {
    let pubkey_block = pubkey
        .map(|pk| PREFS_PUBKEY_LINE_TMPL.replace("{pubkey}", &xml::escape(pk)))
        .unwrap_or_default();
    PREFS_PLIST_TMPL
        .replace("{gateway_esc}", &xml::escape(gateway))
        .replace("{binary_esc}", &xml::escape(&binary.to_string_lossy()))
        .replace("{pubkey_block}", &pubkey_block)
}

#[cfg(target_os = "macos")]
pub fn build_mobileconfig(binary: &Path, gateway: &str, pubkey: Option<&str>) -> String {
    let pubkey_block = pubkey
        .map(|pk| MOBILECONFIG_PUBKEY_LINE_TMPL.replace("{pubkey}", &xml::escape(pk)))
        .unwrap_or_default();
    MOBILECONFIG_TMPL
        .replace("{inner_payload_identifier}", INNER_PAYLOAD_IDENTIFIER)
        .replace("{outer_payload_identifier}", PAYLOAD_IDENTIFIER)
        .replace("{inner_uuid}", &xml::stable_uuid(INNER_PAYLOAD_IDENTIFIER))
        .replace("{outer_uuid}", &xml::stable_uuid(PAYLOAD_IDENTIFIER))
        .replace("{gateway_esc}", &xml::escape(gateway))
        .replace("{binary_esc}", &xml::escape(&binary.to_string_lossy()))
        .replace("{pubkey_block}", &pubkey_block)
}

#[cfg(target_os = "macos")]
fn validate_gateway(gateway: &str) -> Result<(), String> {
    if gateway.starts_with("http://")
        && !gateway.contains("://127.0.0.1")
        && !gateway.contains("://localhost")
    {
        return Err(format!(
            "gateway url {gateway} uses http:// for a non-loopback host; \
             Cowork rejects this. Use https:// or http://127.0.0.1:<port>."
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn apply(binary: &Path, gateway: &str, pubkey: Option<&str>) -> Result<Vec<String>, String> {
    use std::fs;
    use std::process::Command;

    validate_gateway(gateway)?;

    let plist = build_prefs_plist(binary, gateway, pubkey);
    let tmp_path = std::env::temp_dir().join("systemprompt-cowork.prefs.plist");
    fs::write(&tmp_path, plist.as_bytes())
        .map_err(|e| format!("write {}: {e}", tmp_path.display()))?;

    let user = std::env::var("USER").unwrap_or_default();
    let tmp_str = tmp_path.to_string_lossy();
    let dest_system = MANAGED_PREFS_PATH;
    let dest_user =
        format!("/Library/Managed Preferences/{user}/com.anthropic.claudefordesktop.plist");
    let script = if user.is_empty() {
        format!(
            r#"set -e
mkdir -p "/Library/Managed Preferences"
/usr/bin/install -m 0644 "{tmp_str}" "{dest_system}"
/usr/bin/killall cfprefsd 2>/dev/null || true
"#
        )
    } else {
        format!(
            r#"set -e
mkdir -p "/Library/Managed Preferences" "/Library/Managed Preferences/{user}"
/usr/bin/install -m 0644 "{tmp_str}" "{dest_system}"
/usr/bin/install -m 0644 "{tmp_str}" "{dest_user}"
/usr/bin/killall cfprefsd 2>/dev/null || true
"#
        )
    };

    let status = Command::new("sudo")
        .args(["sh", "-c", &script])
        .status()
        .map_err(|e| format!("sudo sh: {e}"))?;
    let _ = fs::remove_file(&tmp_path);
    if !status.success() {
        return Err(format!(
            "sudo direct-write exited with {}. Re-run `systemprompt-cowork install --apply` and \
             approve the sudo prompt, or try `--apply-mobileconfig` for the MDM/System-Settings \
             path.",
            status.code().unwrap_or(-1)
        ));
    }

    Ok(apply_summary(dest_system, &dest_user, &user, gateway, binary))
}

#[cfg(target_os = "macos")]
fn apply_summary(
    dest_system: &str,
    dest_user: &str,
    user: &str,
    gateway: &str,
    binary: &Path,
) -> Vec<String> {
    let mut summary = Vec::with_capacity(16);
    summary.push(format!("wrote: {dest_system}"));
    if !user.is_empty() {
        summary.push(format!("wrote: {dest_user}"));
    }
    summary.push(format!("gateway:           {gateway}"));
    summary.push(format!("credential helper: {}", binary.display()));
    summary.push("restarted cfprefsd (managed prefs picked up on next app launch)".into());
    summary.push(
        "Verify: defaults read /Library/Managed\\ Preferences/com.anthropic.claudefordesktop"
            .into(),
    );
    summary.push("Fully quit Cowork (Cmd+Q) and relaunch to pick up the new policy.".into());
    summary.push(String::new());
    summary.push("Next step — configure an upstream model at the gateway:".into());
    summary.push("  Pointing Cowork at the gateway is half the flow. The gateway must also".into());
    summary.push("  have a provider+model route that accepts the model id Cowork requests".into());
    summary
        .push("  (e.g. claude-sonnet-4-6). If the gateway rejects the model, Cowork shows:".into());
    summary.push(
        "    \"There's an issue with the selected model (<id>). It may not exist...\"".into(),
    );
    summary
        .push("  Configure upstream providers + model mappings at services/ai/config.yaml".into());
    summary.push(
        "  (API keys via env vars: ANTHROPIC_API_KEY / OPENAI_API_KEY / GEMINI_API_KEY)".into(),
    );
    summary.push("  and restart the gateway.".into());
    summary
}

#[cfg(target_os = "macos")]
pub fn apply_mobileconfig(
    binary: &Path,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<Vec<String>, String> {
    use std::fs;
    use std::process::Command;

    validate_gateway(gateway)?;

    let mobileconfig = build_mobileconfig(binary, gateway, pubkey);
    let out_path = std::env::temp_dir().join("systemprompt-cowork.mobileconfig");
    fs::write(&out_path, mobileconfig.as_bytes())
        .map_err(|e| format!("write {}: {e}", out_path.display()))?;

    let opened = Command::new("open").arg(&out_path).status();

    let mut summary = Vec::with_capacity(5);
    summary.push(format!("wrote mobileconfig: {}", out_path.display()));
    summary.push(format!("payload identifier: {PAYLOAD_IDENTIFIER}"));
    match opened {
        Ok(s) if s.success() => summary.push(
            "opened System Settings → Profiles — approve the profile there, then relaunch Cowork."
                .into(),
        ),
        _ => summary.push(format!(
            "could not auto-open System Settings; double-click {} manually.",
            out_path.display()
        )),
    }
    summary
        .push("For fleet deployment, distribute this file via Jamf/Intune/Mosyle instead.".into());
    Ok(summary)
}

#[cfg(target_os = "macos")]
pub fn remove_profile() -> Result<bool, String> {
    use std::process::Command;
    let user = std::env::var("USER").unwrap_or_default();
    let user_path =
        format!("/Library/Managed Preferences/{user}/com.anthropic.claudefordesktop.plist");
    let sys_exists = Path::new(MANAGED_PREFS_PATH).exists();
    let user_exists = !user.is_empty() && Path::new(&user_path).exists();

    let _ = Command::new("sudo")
        .args(["profiles", "remove", "-identifier", PAYLOAD_IDENTIFIER])
        .status();

    if !sys_exists && !user_exists {
        return Ok(false);
    }

    let script = if user_exists {
        format!(
            r#"rm -f "{MANAGED_PREFS_PATH}" "{user_path}"
/usr/bin/killall cfprefsd 2>/dev/null || true
"#
        )
    } else {
        format!(
            r#"rm -f "{MANAGED_PREFS_PATH}"
/usr/bin/killall cfprefsd 2>/dev/null || true
"#
        )
    };
    let status = Command::new("sudo")
        .args(["sh", "-c", &script])
        .status()
        .map_err(|e| format!("sudo sh: {e}"))?;
    if !status.success() {
        return Err(format!(
            "sudo direct-remove exited with {}",
            status.code().unwrap_or(-1)
        ));
    }
    Ok(true)
}

#[cfg(not(target_os = "macos"))]
pub fn apply(
    _binary: &Path,
    _gateway: &str,
    _pubkey: Option<&str>,
) -> Result<Vec<String>, String> {
    Err("--apply on macOS must be run from a macOS binary".into())
}
