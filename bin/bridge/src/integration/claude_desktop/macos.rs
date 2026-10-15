//! Claude Desktop managed-preferences install/probe on macOS.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::process::Command;

pub(super) use super::macos_read::read_domain;
use super::shared::ProfileGenInputs;
use crate::install::approval::ApprovalRefusal;
use crate::install::xml::escape;
use crate::integration::host_app::{GeneratedProfile, HostAppError, ProfileInstalled};

const MANAGED_PREFS_ROOT: &str = "/Library/Managed Preferences";
const PROFILE_TMPL: &str = include_str!("templates/claude_desktop_profile.mobileconfig.tmpl");

// Why: the managed preferences domain is root-owned; every rewrite goes
// through the administrator prompt.
pub(super) const fn update_needs_approval(
    _profile_source: Option<&str>,
    _env: &crate::integration::host_app::ProbeEnv,
) -> bool {
    true
}


pub(super) fn candidates(domain: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(user) = std::env::var("USER")
        && !user.is_empty()
    {
        out.push(
            PathBuf::from(MANAGED_PREFS_ROOT)
                .join(&user)
                .join(format!("{domain}.plist")),
        );
    }
    out.push(PathBuf::from(MANAGED_PREFS_ROOT).join(format!("{domain}.plist")));
    out
}

pub(super) fn list_claude_processes() -> Result<Vec<String>, crate::sysproc::SysprocError> {
    let mut hits: Vec<String> = crate::sysproc::list_processes()?
        .into_iter()
        .filter_map(|p| {
            let name_lower = p.name.to_ascii_lowercase();
            let path_lower = p
                .path
                .as_deref()
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            let matches = path_lower.contains("/claude.app/")
                || path_lower.ends_with("/claude")
                || name_lower.contains("claude helper")
                || path_lower.contains("claude helper");
            let is_code = name_lower.contains("claude code") || path_lower.contains("claude code");
            if matches && !is_code {
                Some(if path_lower.is_empty() {
                    name_lower
                } else {
                    path_lower
                })
            } else {
                None
            }
        })
        .collect();
    hits.sort();
    hits.dedup();
    Ok(hits)
}

pub(super) fn write_profile(inputs: &ProfileGenInputs) -> Result<GeneratedProfile, HostAppError> {
    let uuids = crate::integration::generated_profile::profile_uuids();
    let xml = render_profile(inputs, &uuids.payload, &uuids.profile)?;
    let path = crate::integration::generated_profile::write_persistent(
        "claude-desktop.mobileconfig",
        xml.as_bytes(),
    )?;

    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: xml.len(),
        payload_uuid: uuids.payload,
        profile_uuid: uuids.profile,
    })
}

pub(super) fn install_profile(path: &str) -> Result<ProfileInstalled, HostAppError> {
    let status = Command::new("/usr/bin/open").arg(path).status()?;
    if !status.success() {
        return Err(std::io::Error::other("could not open the configuration profile").into());
    }
    // Why: macOS registers the profile on `open` but shows it only inside
    // System Settings › Device Management; opening the pane puts the Install
    // button in front of the user while the 8-minute window runs.
    let pane = Command::new("/usr/bin/open")
        .arg(crate::integration::app_launch::DEVICE_MANAGEMENT_URL)
        .status();
    if !pane.as_ref().is_ok_and(std::process::ExitStatus::success) {
        tracing::warn!(?pane, "could not open the Device Management pane");
    }
    Ok(ProfileInstalled::with_warning(
        "Approve within 8 minutes in System Settings › General › Device Management → Claude Desktop profile → Install, then relaunch Claude Desktop.".to_owned(),
    ))
}

pub(super) fn install_profile_unattended(_path: &str) -> Result<ProfileInstalled, HostAppError> {
    Err(ApprovalRefusal::NeedsPrompt {
        reason: "a configuration profile is approved by the user in System Settings; use Repair",
    }
    .into())
}


#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "these braces are template placeholders substituted with str::replace, not format args"
)]
fn render_profile(
    inputs: &ProfileGenInputs,
    payload_uuid: &str,
    profile_uuid: &str,
) -> std::io::Result<String> {
    let policy = crate::install::mdm::policy::desktop_policy(inputs, None)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    Ok(PROFILE_TMPL
        .replace("{profile_uuid}", &escape(profile_uuid))
        .replace("{payload_uuid}", &escape(payload_uuid))
        .replace(
            "{policy_body}",
            &crate::install::mdm::policy::plist_body(&policy, "        "),
        ))
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "return type is fixed by the cross-platform HostApp trait"
)]
pub(super) fn remove_profile() -> Result<crate::integration::host_app::ProfileRemoval, HostAppError>
{
    Ok(
        crate::integration::host_app::ProfileRemoval::ManualStepRequired {
            instruction:
                "Remove the Claude Desktop configuration profile under System Settings \u{203a} \
                      General \u{203a} Device Management."
                    .to_owned(),
        },
    )
}
