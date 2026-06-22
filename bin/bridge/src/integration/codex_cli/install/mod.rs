//! Codex CLI managed-profile installer: renders the bridge-owned config block
//! (`.toml`, or `.mobileconfig` on macOS) and merges it into the system-scope
//! config, preserving every user-authored key.

mod merge;
mod render;

use std::io::Write;
use std::path::Path;

use super::config;
use crate::integration::host_app::{GeneratedProfile, ProfileGenInputs};

// Pid + monotonic counter keep concurrent stagers in the shared temp dir from
// racing on the same `File::create` path.
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
                    .as_deref()
                    .map_or_else(|| "systemprompt".into(), |p| p.display().to_string()),
            ),
        ))
    }
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
