//! Claude Desktop policy install/probe on Windows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

use std::io::Write;

use super::shared::{
    API_KEY_KEY, DESKTOP_DOMAIN, DomainRead, KEYS_OF_INTEREST, ProfileGenInputs, make_uuids,
    redact_if_sensitive, unique_stem,
};
use crate::config::store::{PolicyWrite, clear_managed_claude_policy, managed_policy_store};
use crate::integration::host_app::{GeneratedProfile, ProfileRemoval};
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

pub(super) fn list_claude_processes() -> Vec<String> {
    let mut hits: Vec<String> = crate::sysproc::list_processes()
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
    hits
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
    let dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    std::fs::create_dir_all(&dir)?;
    let (payload_uuid, profile_uuid) = make_uuids();
    let path = dir.join(format!("claude-bridge-{}.reg", unique_stem()));

    let body = super::reg_profile::render_reg(winproc::is_elevated(), inputs);
    std::fs::File::create(&path)?.write_all(body.as_bytes())?;

    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: body.len(),
        payload_uuid,
        profile_uuid,
    })
}

pub(super) fn install_profile(path: &str) -> std::io::Result<()> {
    let elevated = winproc::is_elevated();
    tracing::info!(path, elevated, "installing Claude Desktop profile");
    let body = std::fs::read_to_string(path)?;
    let entries = crate::install::reg_values::parse_reg_entries(&body);
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
    // Why: the policy is already written and verified above. A missing
    // org-plugins directory is a distinct, later failure; reporting it as
    // the profile install failing would send the operator to re-run a step
    // that succeeded.
    if let Err(e) = require_org_plugins_provisioned(elevated) {
        return Err(std::io::Error::other(format!(
            "policy written to {} and read back, but org-plugins is not usable: {e}",
            crate::config::store::hive_for(elevated).label()
        )));
    }
    tracing::info!(
        value_count = entries.len(),
        "Claude Desktop profile installed"
    );
    Ok(())
}

fn install_profile_elevated(path: &str) -> std::io::Result<()> {
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
    };
    let receipt = crate::install::elevated_job::elevate_and_run(&stage_dir, &job)?;
    receipt.require("policy", std::path::Path::new(path))?;
    tracing::info!(
        path,
        "Claude Desktop profile installed through an elevated write"
    );
    Ok(())
}

fn require_org_plugins_provisioned(elevated: bool) -> std::io::Result<()> {
    let org = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user()?;
    if elevated {
        crate::install::elevated_job::provision_org_plugins(&org.path, &org.grant_user).map_err(
            |e| {
                tracing::error!(error = %e, "org-plugins provisioning failed");
                std::io::Error::other(format!("org-plugins provisioning failed: {e}"))
            },
        )?;
        crate::windows_acl::verify_modify_tree(&org.path)
    } else if org.path.is_dir() {
        Ok(())
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
