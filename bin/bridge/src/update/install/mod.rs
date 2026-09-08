//! Platform-specific swap of a verified artifact over the running install.
//!
//! Every backend takes the same contract: it is handed a path to bytes whose
//! digest has already been checked, and it either replaces the install or
//! returns an error having changed nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use crate::update::error::UpdateError;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub(crate) fn apply(
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows", target_os = "linux")),
        expect(unused_variables, reason = "no installer exists for other platforms")
    )]
    staged: &Path,
) -> Result<PathBuf, UpdateError> {
    #[cfg(target_os = "macos")]
    {
        macos::apply(staged)
    }
    #[cfg(target_os = "windows")]
    {
        windows::apply(staged)
    }
    #[cfg(target_os = "linux")]
    {
        linux::apply(staged)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err(UpdateError::UnsupportedPlatform)
    }
}

#[cfg_attr(
    not(target_os = "windows"),
    expect(
        clippy::missing_const_for_fn,
        reason = "empty only off Windows, where the body calls a non-const fn"
    )
)]
pub(crate) fn sweep_leftovers() {
    #[cfg(target_os = "windows")]
    {
        windows::sweep_leftovers();
    }
}

// Why: Permission bits do not account for macOS SIP or read-only mounts.
pub(crate) fn probe_writable(path: &Path, hint: &str) -> Result<(), UpdateError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let probe = crate::fsutil::temp_path_for(&parent.join(".update-probe"));
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            if let Err(e) = std::fs::remove_file(&probe) {
                tracing::debug!(error = %e, path = %probe.display(), "update: probe cleanup failed");
            }
            Ok(())
        },
        Err(_) => Err(UpdateError::NotWritable {
            path: path.to_path_buf(),
            hint: hint.to_owned(),
        }),
    }
}

pub(crate) fn installed_path() -> Result<PathBuf, UpdateError> {
    let exe = running_exe()?;
    #[cfg(target_os = "macos")]
    {
        Ok(exe
            .ancestors()
            .find(|p| p.extension().is_some_and(|e| e == "app"))
            .map_or_else(|| exe.clone(), Path::to_path_buf))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(exe)
    }
}

pub(crate) fn running_exe() -> Result<PathBuf, UpdateError> {
    let exe = std::env::current_exe().map_err(|e| UpdateError::LocateInstall {
        what: "executable",
        detail: e.to_string(),
    })?;
    Ok(std::fs::canonicalize(&exe).unwrap_or(exe))
}
