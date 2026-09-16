//! Codex CLI managed-profile installer: renders the bridge-owned config block
//! (`.toml`, or `.mobileconfig` on macOS) and merges it into the system-scope
//! config, preserving every user-authored key.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod merge;
mod render;

use std::path::Path;

use super::config;
use crate::integration::generated_profile;
use crate::integration::host_app::{GeneratedProfile, ProfileGenInputs, ProfileRemoval};

pub(super) fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    let uuids = generated_profile::profile_uuids();
    let toml_text = render::managed_toml(inputs)?;

    if cfg!(target_os = "macos") {
        let xml = render::mobileconfig(&toml_text, &uuids.payload, &uuids.profile);
        let path = generated_profile::write("codex-bridge", ".mobileconfig", xml.as_bytes())?;
        Ok(GeneratedProfile {
            path: path.display().to_string(),
            bytes: xml.len(),
            payload_uuid: uuids.payload,
            profile_uuid: uuids.profile,
        })
    } else {
        let path =
            generated_profile::write("codex-bridge-managed_config", ".toml", toml_text.as_bytes())?;
        Ok(GeneratedProfile {
            path: path.display().to_string(),
            bytes: toml_text.len(),
            payload_uuid: uuids.payload,
            profile_uuid: uuids.profile,
        })
    }
}

// Why: macOS requires manual approval of profiles opened in System Settings;
// unattended installation needs MDM.
#[cfg(target_os = "macos")]
fn notify_profile_pending() {
    crate::user_alert::alert_user(
        &format!("{} needs approval", crate::brand::brand().app_name),
        "Approve the Codex configuration profile in System Settings → General → Device \
         Management to finish installing it.",
    );
}

#[cfg(not(target_os = "macos"))]
const fn notify_profile_pending() {}

pub(super) fn install_profile(generated_path: &str) -> std::io::Result<()> {
    if cfg!(target_os = "macos") {
        std::process::Command::new("/usr/bin/open")
            .args(["-g", generated_path])
            .status()?;
        notify_profile_pending();
        return Ok(());
    }

    let target = config::managed_config_path();
    let parent = target.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("cannot resolve parent for {}", target.display()),
        )
    })?;

    if cfg!(target_os = "windows") {
        std::fs::create_dir_all(parent)?;
        merge::install(generated_path.as_ref(), &target)?;
        return generated_profile::consume(generated_path);
    }

    if std::fs::create_dir_all(parent).is_ok() && writable(parent) {
        merge::install(generated_path.as_ref(), &target)?;
        generated_profile::consume(generated_path)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "{} is admin-owned. Re-run as root: sudo {} bridge codex install",
                parent.display(),
                std::env::current_exe()
                    .ok()
                    .as_deref()
                    .map_or_else(|| "systemprompt".into(), |p| p.display().to_string()),
            ),
        ))
    }
}

pub(super) fn remove_profile() -> std::io::Result<ProfileRemoval> {
    if cfg!(target_os = "macos") {
        return Ok(ProfileRemoval::ManualStepRequired {
            instruction: "Remove the Codex CLI configuration profile under System Settings › \
                          General › Device Management."
                .to_owned(),
        });
    }
    let target = config::managed_config_path();
    let removed = merge::uninstall(&target)?;
    Ok(if removed {
        ProfileRemoval::Removed {
            path: Some(target.display().to_string()),
        }
    } else {
        ProfileRemoval::NothingToRemove
    })
}

fn writable(path: &Path) -> bool {
    let probe = path.join(format!(
        ".{}-write-test-{}",
        crate::brand::brand().binary_name,
        std::process::id()
    ));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            crate::fsutil::remove_leftover_file(&probe);
            true
        },
        Err(_) => false,
    }
}
