//! macOS MDM configuration-profile payloads and managed-preferences plist
//! rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]

use std::path::Path;

pub use super::macos_payload::{build_mobileconfig, build_prefs_plist};
use super::{MdmError, MdmPayloadInputs};

pub(crate) const PAYLOAD_IDENTIFIER: &str = "io.systemprompt.bridge.mdm";
pub(super) const INNER_PAYLOAD_IDENTIFIER: &str = "io.systemprompt.bridge.mdm.inference";
pub(crate) const MANAGED_PREFS_PATH: &str =
    "/Library/Managed Preferences/com.anthropic.claudefordesktop.plist";

fn validate_gateway(gateway: &str) -> Result<(), MdmError> {
    if gateway.starts_with("http://")
        && !gateway.contains("://127.0.0.1")
        && !gateway.contains("://localhost")
    {
        return Err(MdmError::InsecureGateway {
            gateway: gateway.to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn apply(
    mcp: &MdmPayloadInputs<'_>,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<Vec<String>, MdmError> {
    use std::fs;

    validate_gateway(gateway)?;

    let plist = build_prefs_plist(mcp, gateway, pubkey);
    let tmp_path =
        std::env::temp_dir().join(format!("{}.prefs.plist", crate::brand::brand().binary_name));
    fs::write(&tmp_path, plist.as_bytes()).map_err(|e| MdmError::Io {
        action: "write",
        path: tmp_path.clone(),
        source: e,
    })?;

    let user = std::env::var("USER").unwrap_or_default();
    let tmp_str = tmp_path.to_string_lossy();
    let dest_system = MANAGED_PREFS_PATH;
    let dest_user =
        format!("/Library/Managed Preferences/{user}/com.anthropic.claudefordesktop.plist");

    // Why: skip elevation when the on-disk plist already matches. The read is
    // only possible because Managed Preferences is world-readable.
    let existing_matches = fs::read(dest_system).is_ok_and(|b| b == plist.as_bytes())
        && (user.is_empty() || fs::read(&dest_user).is_ok_and(|b| b == plist.as_bytes()));

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

    let result = if existing_matches {
        Ok(())
    } else {
        crate::install::elevate::run_privileged(
            &script,
            "Astound Bridge needs administrator privileges to install the Claude Desktop managed preferences.",
        )
    };
    _ = fs::remove_file(&tmp_path);
    result.map_err(|e| MdmError::ApplyElevation {
        binary: crate::brand::brand().binary_name,
        source: e,
    })?;

    Ok(apply_summary(dest_system, &dest_user, &user, gateway))
}

fn apply_summary(
    dest_system: &str,
    dest_user: &str,
    user: &str,
    inference_base_url: &str,
) -> Vec<String> {
    let mut summary = Vec::with_capacity(16);
    summary.push(format!("wrote: {dest_system}"));
    if !user.is_empty() {
        summary.push(format!("wrote: {dest_user}"));
    }
    summary.push(format!(
        "inferenceGatewayBaseUrl: {inference_base_url}  (local proxy)"
    ));
    summary.push("auth: inferenceGatewayApiKey = loopback secret (proxy-bound)".into());
    summary.push("restarted cfprefsd (managed prefs picked up on next app launch)".into());
    summary.push(
        "Verify: defaults read /Library/Managed\\ Preferences/com.anthropic.claudefordesktop"
            .into(),
    );
    summary.push("Fully quit Bridge (Cmd+Q) and relaunch to pick up the new policy.".into());
    summary.push(String::new());
    summary.push("Next step — configure an upstream model at the gateway:".into());
    summary.push("  Pointing Bridge at the gateway is half the flow. The gateway must also".into());
    summary.push("  have a provider+model route that accepts the model id Bridge requests".into());
    summary
        .push("  (e.g. claude-sonnet-4-6). If the gateway rejects the model, Bridge shows:".into());
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

pub(crate) fn apply_mobileconfig(
    mcp: &MdmPayloadInputs<'_>,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<Vec<String>, MdmError> {
    use std::fs;
    use std::process::Command;

    validate_gateway(gateway)?;

    let mobileconfig = build_mobileconfig(mcp, gateway, pubkey);
    let out_path = std::env::temp_dir().join(format!(
        "{}.mobileconfig",
        crate::brand::brand().binary_name
    ));
    fs::write(&out_path, mobileconfig.as_bytes()).map_err(|e| MdmError::Io {
        action: "write",
        path: out_path.clone(),
        source: e,
    })?;

    // Why: `-g` opens System Settings without switching focus, avoiding a
    // wry/muda/objc2 weak-ref teardown crash on the bridge window (see
    // integration/claude_desktop/macos.rs::install_profile for the full story).
    let opened = Command::new("open").arg("-g").arg(&out_path).status();

    let mut summary = Vec::with_capacity(5);
    summary.push(format!("wrote mobileconfig: {}", out_path.display()));
    summary.push(format!("payload identifier: {PAYLOAD_IDENTIFIER}"));
    match opened {
        Ok(s) if s.success() => summary.push(
            "opened System Settings → Profiles — approve the profile there, then relaunch Bridge."
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

pub(crate) fn remove_profile() -> Result<bool, MdmError> {
    let user = std::env::var("USER").unwrap_or_default();
    let user_path =
        format!("/Library/Managed Preferences/{user}/com.anthropic.claudefordesktop.plist");
    let sys_exists = Path::new(MANAGED_PREFS_PATH).exists();
    let user_exists = !user.is_empty() && Path::new(&user_path).exists();

    if !sys_exists && !user_exists {
        return Ok(false);
    }

    let script = format!(
        r"set -e
/usr/bin/profiles remove -identifier {PAYLOAD_IDENTIFIER} 2>/dev/null || true
{rm_lines}
/usr/bin/killall cfprefsd 2>/dev/null || true
",
        rm_lines = if user_exists {
            format!(r#"rm -f "{MANAGED_PREFS_PATH}" "{user_path}""#)
        } else {
            format!(r#"rm -f "{MANAGED_PREFS_PATH}""#)
        },
    );
    crate::install::elevate::run_privileged(
        &script,
        "Astound Bridge needs administrator privileges to remove the Claude Desktop managed preferences.",
    )?;
    Ok(true)
}
