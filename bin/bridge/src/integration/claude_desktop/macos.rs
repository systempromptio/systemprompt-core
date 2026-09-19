//! Claude Desktop managed-preferences install/probe on macOS.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]

use std::path::{Path, PathBuf};
use std::process::Command;

use super::shared::{
    API_KEY_KEY, DomainRead, KEYS_OF_INTEREST, ProfileGenInputs, redact_if_sensitive,
};
use crate::install::xml::escape;
use crate::integration::host_app::{GeneratedProfile, ProfileInstalled};

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

pub(super) fn read_domain(domain: &str) -> DomainRead {
    let mut out = DomainRead::default();

    let plist_path = candidates(domain).into_iter().find(|p| p.exists());

    if let Some(path) = plist_path.as_ref() {
        out.source_path = Some(path.display().to_string());
    }

    let plist_json = plist_path
        .as_deref()
        .and_then(read_plist_as_json)
        .unwrap_or(serde_json::Value::Null);

    for key in KEYS_OF_INTEREST {
        if let Some(raw) = read_key_raw(&plist_json, domain, key) {
            if *key == API_KEY_KEY {
                out.api_key_fp = Some(crate::proxy::secret::fingerprint(raw.trim()));
            }
            out.keys
                .insert((*key).to_owned(), redact_if_sensitive(key, raw));
        }
    }

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

pub(super) fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    let uuids = crate::integration::generated_profile::profile_uuids();
    let xml = render_profile(inputs, &uuids.payload, &uuids.profile)?;
    let path = crate::integration::generated_profile::write(
        "claude-bridge",
        ".mobileconfig",
        xml.as_bytes(),
    )?;

    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: xml.len(),
        payload_uuid: uuids.payload,
        profile_uuid: uuids.profile,
    })
}

pub(super) fn install_profile(path: &str) -> std::io::Result<ProfileInstalled> {
    Command::new("/usr/bin/open").args(["-g", path]).status()?;
    Ok(ProfileInstalled::ok())
}

pub(super) fn install_profile_unattended(_path: &str) -> std::io::Result<ProfileInstalled> {
    Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "a configuration profile is approved by the user in System Settings; use Repair",
    ))
}

fn candidates(domain: &str) -> Vec<PathBuf> {
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

fn read_plist_as_json(path: &Path) -> Option<serde_json::Value> {
    let output = Command::new("/usr/bin/plutil")
        .arg("-convert")
        .arg("json")
        .arg("-o")
        .arg("-")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

fn read_key_raw(plist_json: &serde_json::Value, _domain: &str, key: &str) -> Option<String> {
    if let Some(val) = plist_json.get(key) {
        return Some(format_plist_value(val));
    }

    let raw = crate::config::store::managed_policy_store()
        .read_managed_policy(key)
        .ok()
        .flatten()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_owned())
}

// Why: an array of objects (`allowedWorkspaceFolders`, `managedMcpServers`)
// rendered through a strings-only join printed as empty, which hid a plist
// whose entries Claude Desktop was dropping as malformed.
fn format_plist_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) if items.iter().all(serde_json::Value::is_string) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    }
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
    let models = if inputs.models.is_empty() {
        super::shared::default_models()
    } else {
        inputs.models.clone()
    };
    let models_json = serde_json::to_string(&models)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let policy = crate::install::mdm::policy::claude_desktop_policy(
        &crate::install::mdm::policy::PolicyInputs {
            base_url: &inputs.gateway_base_url,
            host_token: &inputs.host_token,
            models: Some(models_json),
            headers: &inputs.headers,
            egress_allowed_hosts: None,
            org_uuid: inputs.organization_uuid.as_deref(),
            mcp_servers: inputs.mcp_servers.as_deref(),
        },
    )
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
pub(super) fn remove_profile() -> std::io::Result<crate::integration::host_app::ProfileRemoval> {
    Ok(
        crate::integration::host_app::ProfileRemoval::ManualStepRequired {
            instruction:
                "Remove the Claude Desktop configuration profile under System Settings \u{203a} \
                      General \u{203a} Device Management."
                    .to_owned(),
        },
    )
}
