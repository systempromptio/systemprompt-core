//! macOS MDM configuration-profile payloads and managed-preferences plist
//! rendering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]

use std::path::Path;

pub use super::macos_payload::{build_bridge_prefs_plist, build_mobileconfig, build_prefs_plist};
pub(crate) use super::macos_remove::remove_profile;
use super::{MdmError, MdmPayloadInputs};

pub(crate) const PAYLOAD_IDENTIFIER: &str = "io.systemprompt.bridge.mdm";
pub(super) const INNER_PAYLOAD_IDENTIFIER: &str = "io.systemprompt.bridge.mdm.inference";
pub(super) const BRIDGE_PAYLOAD_IDENTIFIER: &str = "io.systemprompt.bridge.mdm.policy";
pub(crate) const MANAGED_PREFS_PATH: &str =
    "/Library/Managed Preferences/com.anthropic.claudefordesktop.plist";

pub(super) fn bridge_prefs_path() -> String {
    format!(
        "/Library/Managed Preferences/{}.plist",
        crate::config::store::bridge_policy_domain()
    )
}

fn validate_gateway(gateway: &str) -> Result<(), MdmError> {
    let url = url::Url::parse(gateway).map_err(|e| MdmError::InvalidConfig(e.to_string()))?;
    let loopback = match url.host() {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    };
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
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
) -> Result<super::MdmApplication, MdmError> {
    use std::fs;

    validate_gateway(gateway)?;

    let plist = build_prefs_plist(mcp, gateway)?;
    let staging = tempfile::tempdir().map_err(|source| MdmError::Io {
        action: "create staging directory",
        path: std::env::temp_dir(),
        source,
    })?;
    let tmp_path = staging.path().join("claude.plist");
    crate::fsutil::atomic_write_0600(&tmp_path, plist.as_bytes()).map_err(|source| {
        MdmError::Io {
            action: "stage policy",
            path: tmp_path.clone(),
            source,
        }
    })?;
    let bridge_plist = pubkey.map(build_bridge_prefs_plist).transpose()?;
    let bridge_tmp = staging.path().join("bridge.plist");
    if let Some(body) = &bridge_plist {
        crate::fsutil::atomic_write_0600(&bridge_tmp, body.as_bytes()).map_err(|source| {
            MdmError::Io {
                action: "stage trust",
                path: bridge_tmp.clone(),
                source,
            }
        })?;
    }
    let user = std::env::var("USER").map_err(|e| MdmError::InvalidConfig(format!("USER: {e}")))?;
    if user.is_empty() || user.contains('/') || user == "." || user == ".." {
        return Err(MdmError::InvalidConfig(
            "USER cannot identify a managed-preferences directory".to_owned(),
        ));
    }
    let dest_system = MANAGED_PREFS_PATH;
    let dest_user =
        format!("/Library/Managed Preferences/{user}/com.anthropic.claudefordesktop.plist");
    let bridge_dest = bridge_prefs_path();
    let mut writes = vec![
        (&tmp_path, Path::new(dest_system), plist.as_bytes()),
        (&tmp_path, Path::new(&dest_user), plist.as_bytes()),
    ];
    if let Some(body) = &bridge_plist {
        writes.push((&bridge_tmp, Path::new(&bridge_dest), body.as_bytes()));
    }
    let mut script = "set -e\n".to_owned();
    let mut changed = false;
    for (source, target, bytes) in &writes {
        match fs::read(target) {
            Ok(current) if current == *bytes => continue,
            Ok(_) => {},
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                ) => {},
            Err(source) => {
                return Err(MdmError::Io {
                    action: "read policy",
                    path: target.to_path_buf(),
                    source,
                });
            },
        }
        changed = true;
        let quote = crate::install::elevation_script::shell_quote;
        let parent = target
            .parent()
            .ok_or_else(|| MdmError::InvalidConfig("policy path has no parent".to_owned()))?;
        script.push_str(&format!(
            "mkdir -p {}\n/usr/bin/install -m 0644 {} {}\n",
            quote(&parent.to_string_lossy()),
            quote(&source.to_string_lossy()),
            quote(&target.to_string_lossy())
        ));
    }
    if changed {
        script.push_str("/usr/bin/killall cfprefsd\n");
        crate::install::elevate::run_privileged(
            &script,
            "Bridge needs administrator privileges to install the Claude Desktop managed preferences.",
        )?;
    }
    let mut files = Vec::new();
    for (_, target, bytes) in writes {
        let receipt = crate::fsutil::FileReceipt::verify(target, bytes)
            .map_err(|source| MdmError::Io {
                action: "verify policy",
                path: target.to_path_buf(),
                source,
            })
            .map_err(|source| MdmError::Partial {
                completed: super::MdmApplication {
                    files: files.clone(),
                    ..Default::default()
                },
                source: Box::new(source),
            })?;
        files.push(receipt);
    }

    Ok(super::MdmApplication {
        lines: apply_summary(dest_system, &dest_user, &user, gateway, changed),
        files,
        policies: Vec::new(),
    })
}

fn apply_summary(
    dest_system: &str,
    dest_user: &str,
    user: &str,
    inference_base_url: &str,
    changed: bool,
) -> Vec<String> {
    let mut summary = Vec::with_capacity(16);
    summary.push(format!("verified: {dest_system}"));
    if !user.is_empty() {
        summary.push(format!("verified: {dest_user}"));
    }
    summary.push(format!(
        "inferenceGatewayBaseUrl: {inference_base_url}  (local proxy)"
    ));
    summary.push("auth: inferenceGatewayApiKey = loopback secret (proxy-bound)".into());
    if changed {
        summary.push("restarted cfprefsd (managed prefs picked up on next app launch)".into());
    }
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
    use std::process::Command;

    validate_gateway(gateway)?;

    let mobileconfig = build_mobileconfig(mcp, gateway, pubkey)?;
    let out_path = std::env::temp_dir().join(format!(
        "{}.mobileconfig",
        crate::brand::brand().binary_name
    ));
    crate::fsutil::atomic_write_0600(&out_path, mobileconfig.as_bytes()).map_err(|e| {
        MdmError::Io {
            action: "write",
            path: out_path.clone(),
            source: e,
        }
    })?;

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
