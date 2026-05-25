//! Codex CLI managed-profile installer.
//!
//! `write_profile` renders the bridge-owned config block — as a `.toml` file
//! on Linux/Windows or a signed `.mobileconfig` on macOS — and
//! `install_profile` either hands it to the system installer (macOS) or merges
//! it into the documented system-scope config path (`/etc/codex/config.toml`),
//! preserving every user-authored key.

mod merge;
mod render;

use std::io::Write;
use std::path::Path;

use super::config;
use crate::integration::host_app::{GeneratedProfile, ProfileGenInputs};

pub(super) fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    let dir = std::env::temp_dir().join("systemprompt-bridge");
    std::fs::create_dir_all(&dir)?;
    let (payload_uuid, profile_uuid) = config::make_uuids();

    let toml_text = render::managed_toml(inputs)?;

    if cfg!(target_os = "macos") {
        let path = dir.join(format!("codex-bridge-{}.mobileconfig", config::now_unix()));
        let xml = render::mobileconfig(&toml_text, &payload_uuid, &profile_uuid);
        std::fs::File::create(&path)?.write_all(xml.as_bytes())?;
        Ok(GeneratedProfile {
            path: path.display().to_string(),
            bytes: xml.len(),
            payload_uuid,
            profile_uuid,
        })
    } else {
        let path = dir.join(format!(
            "codex-bridge-{}-managed_config.toml",
            config::now_unix()
        ));
        std::fs::File::create(&path)?.write_all(toml_text.as_bytes())?;
        Ok(GeneratedProfile {
            path: path.display().to_string(),
            bytes: toml_text.len(),
            payload_uuid,
            profile_uuid,
        })
    }
}

pub(super) fn install_profile(generated_path: &str) -> std::io::Result<()> {
    if cfg!(target_os = "macos") {
        std::process::Command::new("/usr/bin/open")
            .arg(generated_path)
            .status()?;
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
        return merge::install(generated_path.as_ref(), &target);
    }

    if std::fs::create_dir_all(parent).is_ok() && writable(parent) {
        merge::install(generated_path.as_ref(), &target)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "{} is admin-owned. Re-run as root: sudo {} bridge codex install",
                parent.display(),
                std::env::current_exe()
                    .ok()
                    .as_deref().map_or_else(|| "systemprompt".into(), |p| p.display().to_string()),
            ),
        ))
    }
}

fn writable(path: &Path) -> bool {
    let probe = path.join(format!(
        ".systemprompt-bridge-write-test-{}",
        std::process::id()
    ));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            _ = std::fs::remove_file(&probe);
            true
        },
        Err(_) => false,
    }
}
