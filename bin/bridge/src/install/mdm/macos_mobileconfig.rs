//! Offering the configuration profile to macOS for manual approval.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]

use std::path::PathBuf;

use super::macos::{PAYLOAD_IDENTIFIER, build_mobileconfig, validate_gateway};
use super::{MdmError, MdmPayloadInputs};

pub(crate) fn apply_mobileconfig(
    mcp: &MdmPayloadInputs<'_>,
    gateway: &str,
    pubkey: Option<&str>,
) -> Result<Vec<String>, MdmError> {
    use std::process::Command;

    validate_gateway(gateway)?;

    let mobileconfig = build_mobileconfig(mcp, pubkey)?;
    let file_name = format!("{}.mobileconfig", crate::brand::brand().binary_name);
    let out_path =
        crate::config::paths::write_persistent_profile(&file_name, mobileconfig.as_bytes())
            .map_err(|e| MdmError::Io {
                action: "write",
                path: PathBuf::from(&file_name),
                source: e,
            })?;

    // Why: macOS drops an offered profile that is not approved within about
    // eight minutes, so it is offered in the foreground with the Device
    // Management pane open, never behind the user's back.
    let opened = Command::new("/usr/bin/open").arg(&out_path).status();
    let pane = Command::new("/usr/bin/open")
        .arg(crate::install::mdm::DEVICE_MANAGEMENT_URL)
        .status();
    if !pane.as_ref().is_ok_and(std::process::ExitStatus::success) {
        tracing::warn!(?pane, "could not open the Device Management pane");
    }

    let mut summary = Vec::with_capacity(5);
    summary.push(format!("wrote mobileconfig: {}", out_path.display()));
    summary.push(format!("payload identifier: {PAYLOAD_IDENTIFIER}"));
    match opened {
        Ok(s) if s.success() => summary.push(
            "opened System Settings › Device Management — approve the profile within 8 minutes, \
             then relaunch Bridge."
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
