//! Claude Desktop policy install/probe on Windows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]


mod writer;

use self::writer::install_through_writer;
use super::shared::{
    API_KEY_KEY, DESKTOP_DOMAIN, DomainRead, KEYS_OF_INTEREST, ProfileGenInputs,
    redact_if_sensitive,
};
use crate::config::store::{
    PolicyWrite, clear_managed_claude_policy, machine_claude_policy_keys, managed_policy_store,
};
use crate::integration::host_app::{GeneratedProfile, ProfileInstalled, ProfileRemoval};
use crate::winproc;

pub(super) fn read_domain(domain: &str) -> DomainRead {
    let mut out = DomainRead::default();

    if domain != DESKTOP_DOMAIN {
        return out;
    }

    let Ok(read) = managed_policy_store().read_managed_policy_keys(KEYS_OF_INTEREST) else {
        return out;
    };
    if read.values.is_empty() {
        return out;
    }
    out.source_path = read.source;
    for (name, value) in read.values {
        if name == API_KEY_KEY {
            out.api_key_fp = Some(crate::proxy::secret::fingerprint(value.trim()));
        }
        out.keys
            .insert(name.clone(), redact_if_sensitive(&name, value));
    }
    out
}

// Why: Claude Desktop reads the machine hive whenever it exists, so a
// profile that lives there is rewritten either elevated or through the
// registered policy writer; only when neither applies does an update raise
// the administrator prompt.
pub(super) fn update_needs_approval(
    profile_source: Option<&str>,
    env: &crate::integration::host_app::ProbeEnv,
) -> bool {
    profile_source.is_some_and(|source| source.starts_with("HKLM"))
        && !winproc::is_elevated()
        && !env.policy_writer_ready
}

pub(super) fn list_claude_processes() -> Result<Vec<String>, crate::sysproc::SysprocError> {
    let mut hits: Vec<String> = crate::sysproc::list_processes()?
        .into_iter()
        .filter_map(|p| {
            let lower = p.name.to_ascii_lowercase();
            let is_claude = lower == "claude.exe" || lower.starts_with("claude helper");
            let is_code = lower.contains("claude code") || lower == "claude-code.exe";
            if is_claude && !is_code && !is_cli_image(p.path.as_deref()) {
                Some(p.name)
            } else {
                None
            }
        })
        .collect();
    hits.sort();
    hits.dedup();
    Ok(hits)
}

// Why: Claude Code and Claude Desktop both use claude.exe; distinguish them by
// image path.
fn is_cli_image(path: Option<&str>) -> bool {
    const CLI_MARKERS: [&str; 3] = [r"\.local\bin\", r"\npm\", r"\node_modules\"];

    path.is_some_and(|p| {
        let lower = p.to_ascii_lowercase();
        CLI_MARKERS.iter().any(|m| lower.contains(m))
    })
}

pub(super) fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    let uuids = crate::integration::generated_profile::profile_uuids();
    let body = super::reg_profile::render_reg(winproc::is_elevated(), inputs)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let path =
        crate::integration::generated_profile::write("claude-bridge", ".reg", body.as_bytes())?;

    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: body.len(),
        payload_uuid: uuids.payload,
        profile_uuid: uuids.profile,
    })
}

pub(super) fn install_profile(path: &str) -> std::io::Result<ProfileInstalled> {
    install_profile_with(path, Attendance::Attended)
}

pub(super) fn install_profile_unattended(path: &str) -> std::io::Result<ProfileInstalled> {
    install_profile_with(path, Attendance::Unattended)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Attendance {
    Attended,
    Unattended,
}

fn install_profile_with(path: &str, attendance: Attendance) -> std::io::Result<ProfileInstalled> {
    let elevated = winproc::is_elevated();
    tracing::info!(path, elevated, "installing Claude Desktop profile");
    let body = std::fs::read_to_string(path)?;
    let entries = crate::install::reg_values::parse_reg_entries(&body)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    tracing::info!(
        path,
        parsed_values = entries.len(),
        names = ?entries.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
        "parsed staged registry profile"
    );
    if entries.is_empty() {
        return Err(std::io::Error::other(
            "staged registry profile contained no policy values",
        ));
    }
    // Why: an ordinary process writes the per-user policy, which Claude honours
    // while no machine policy exists. A machine policy that already holds other
    // values shadows anything HKCU says, and only an elevated write can replace
    // it; the staged profile carries this user's secret, so the same values
    // land whichever administrator approves the prompt.
    let outcome = match crate::config::store::write_managed_claude_policy(elevated, &entries) {
        Ok(outcome) => outcome,
        Err(crate::config::store::ConfigStoreError::HiveConflict { differing, .. })
            if !elevated =>
        {
            match install_through_writer(&entries) {
                Ok(Some(installed)) => return Ok(installed),
                Ok(None) => {},
                Err(e) => tracing::warn!(
                    path,
                    error = %e,
                    "the elevated policy writer did not apply the profile; asking for approval"
                ),
            }
            if attendance == Attendance::Unattended {
                tracing::warn!(
                    path,
                    differing = ?differing,
                    "machine policy holds other values; an unattended repair cannot replace it"
                );
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "the machine policy holds other values; replacing it needs administrator \
                     approval — use Repair",
                ));
            }
            tracing::warn!(
                path,
                differing = ?differing,
                "machine policy holds other values; requesting administrator approval to replace it"
            );
            return install_profile_elevated(path);
        },
        Err(e) => {
            tracing::error!(error = %e, path, "managed Claude policy write failed");
            return Err(std::io::Error::other(e.to_string()));
        },
    };
    match outcome.outcome() {
        PolicyWrite::Written(hive) | PolicyWrite::AlreadyVerified(hive) => {
            tracing::info!(hive = hive.label(), "policy written and read back");
        },
        PolicyWrite::SatisfiedByMachine => {
            tracing::info!("HKLM already holds this policy; per-user copy not written");
        },
    }
    // Why: the policy is already written and verified above. Provisioning
    // org-plugins is a distinct, later step and stays fatal when it fails;
    // a verification that cannot run afterwards is a warning, because the
    // directory and its grant are already in place and a failed step would
    // send the operator to re-run work that succeeded.
    let outcome = require_org_plugins_provisioned(elevated).map_err(|e| {
        std::io::Error::other(format!(
            "policy written to {} and read back, but org-plugins is not usable: {e}",
            crate::config::store::hive_for(elevated).label()
        ))
    })?;
    tracing::info!(
        value_count = entries.len(),
        "Claude Desktop profile installed"
    );
    Ok(outcome)
}

fn install_profile_elevated(path: &str) -> std::io::Result<ProfileInstalled> {
    let org = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user()?;
    let stage_dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    std::fs::create_dir_all(&stage_dir)?;
    let job = crate::install::elevated_job::ElevatedJob {
        reg_path: Some(path.to_owned()),
        org_plugins: Some(org),
        clear_values: Vec::new(),
        bridge_values: Vec::new(),
        managed_files: Vec::new(),
        remove_files: Vec::new(),
        private_dirs: Vec::new(),
    };
    let receipt = crate::install::elevated_job::elevate_and_run(&stage_dir, &job)?;
    receipt.require("policy", std::path::Path::new(path))?;
    tracing::info!(
        path,
        "Claude Desktop profile installed through an elevated write"
    );
    Ok(ProfileInstalled::ok())
}

fn require_org_plugins_provisioned(elevated: bool) -> std::io::Result<ProfileInstalled> {
    let org = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user()?;
    if elevated {
        crate::install::elevated_job::provision_org_plugins(&org.path, &org.grant_user).map_err(
            |e| {
                tracing::error!(error = %e, "org-plugins provisioning failed");
                std::io::Error::other(format!("org-plugins provisioning failed: {e}"))
            },
        )?;
        // Why: PermissionDenied is the check's verdict (the unelevated user
        // lacks Modify), not a failure to run it — that must fail the install.
        Ok(match crate::windows_acl::verify_modify_tree(&org.path) {
            Ok(()) => ProfileInstalled::ok(),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return Err(e),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    path = %org.path.display(),
                    "org-plugins provisioned; Modify verification could not run"
                );
                ProfileInstalled::with_warning(format!(
                    "org-plugins provisioned at {} but the Modify check could not run ({e}); \
                     run `doctor` to confirm the grant",
                    org.path.display()
                ))
            },
        })
    } else if org.path.is_dir() {
        Ok(ProfileInstalled::ok())
    } else {
        Err(std::io::Error::other(format!(
            "{} is not provisioned; run install --apply as Administrator",
            org.path.display()
        )))
    }
}

pub(super) fn remove_profile() -> std::io::Result<ProfileRemoval> {
    let elevated = winproc::is_elevated();
    let removed = clear_managed_claude_policy(elevated, KEYS_OF_INTEREST)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    if !elevated {
        let machine_keys = machine_claude_policy_keys(KEYS_OF_INTEREST)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        if !machine_keys.is_empty() {
            return Ok(ProfileRemoval::ManualStepRequired {
                instruction: format!(
                    "HKLM\\{} still holds {}; run `uninstall` as Administrator to remove the \
                     machine policy",
                    crate::cowork_compat::POLICY_SUBKEY,
                    machine_keys.join(", ")
                ),
            });
        }
    }
    Ok(if removed == 0 {
        ProfileRemoval::NothingToRemove
    } else {
        ProfileRemoval::Removed {
            path: Some(format!(
                r"{}\{}",
                crate::config::store::hive_for(elevated).label(),
                crate::cowork_compat::POLICY_SUBKEY
            )),
        }
    })
}
