//! Codex CLI managed-profile installer: renders the bridge-owned config block
//! (`.toml`, or `.mobileconfig` on macOS) and merges it into the system-scope
//! config, preserving every user-authored key.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod merge;
mod render;

use std::io::Write;
use std::path::Path;

use super::config;
use crate::integration::host_app::{GeneratedProfile, ProfileGenInputs, ProfileRemoval};

fn unique_stem() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        config::now_unix(),
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

pub(super) fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    let dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    std::fs::create_dir_all(&dir)?;
    let (payload_uuid, profile_uuid) = config::make_uuids();

    let toml_text = render::managed_toml(inputs)?;

    if cfg!(target_os = "macos") {
        let path = dir.join(format!("codex-bridge-{}.mobileconfig", unique_stem()));
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
            unique_stem()
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

// Why: `open` only *offers* a `.mobileconfig` — macOS parks it in System
// Settings until the user approves it, and since macOS 11 nothing short of MDM
// can apply one unattended. The `-g` below then means the approval sheet never
// comes forward. Without this notice the app logs a successful install, the
// payload silently never lands, and the next probe truthfully reports it
// missing — each part behaving reasonably, combining into total silence.
#[cfg(target_os = "macos")]
fn notify_profile_pending() {
    crate::gui::window::alert_user(
        &format!("{} needs approval", crate::brand::brand().app_name),
        "Approve the Codex configuration profile in System Settings → General → Device \
         Management to finish installing it.",
    );
}

#[cfg(not(target_os = "macos"))]
const fn notify_profile_pending() {}

pub(super) fn install_profile(generated_path: &str) -> std::io::Result<()> {
    if cfg!(target_os = "macos") {
        // Why: see integration/claude_desktop/macos.rs::install_profile —
        // `-g` prevents System Settings from stealing focus, which would
        // trip a wry/muda/objc2 weak-ref teardown crash on the bridge window.
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
            _ = std::fs::remove_file(&probe);
            true
        },
        Err(_) => false,
    }
}
