//! Landing an admin-owned managed config file for a host: direct write first,
//! escalation only when the direct write is refused for permissions.
//!
//! Root shells, CI and MDM-provisioned users are already privileged and must
//! never be prompted; an unchanged body is never written at all, so repeated
//! installs and syncs stay silent. On macOS the escalation is the same
//! `sudo`/`osascript` path the Claude Code policy uses; on Windows it is the
//! UAC child that lands the Claude policy; on Linux the bridge has no
//! prompt to offer and reports what to re-run as root.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedWrite {
    Written,
    Unchanged,
}

pub(crate) fn write_managed_file(
    path: &Path,
    bytes: &[u8],
    prompt: &str,
) -> io::Result<ManagedWrite> {
    if std::fs::read(path).is_ok_and(|existing| existing == bytes) {
        return Ok(ManagedWrite::Unchanged);
    }
    match crate::fsutil::atomic_write_0644(path, bytes) {
        Ok(()) => Ok(ManagedWrite::Written),
        // Why: only permission-denied justifies escalating — anything else
        // (ENOSPC and friends) is a real failure elevation cannot fix.
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            write_elevated(path, bytes, prompt).map(|()| ManagedWrite::Written)
        },
        Err(e) => Err(e),
    }
}

pub(crate) fn remove_managed_file(path: &Path, prompt: &str) -> io::Result<bool> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            remove_elevated(path, prompt).map(|()| true)
        },
        Err(e) => Err(e),
    }
}

#[cfg(target_os = "macos")]
fn write_elevated(path: &Path, bytes: &[u8], prompt: &str) -> io::Result<()> {
    // Why: stage into a user-writable tempdir first — the elevated shell can
    // read it, whereas a heredoc would embed the body in the script itself.
    let staging = tempfile::Builder::new()
        .prefix("systemprompt-managed-")
        .tempdir()?;
    let staged = staging.path().join(path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "managed path has no file name")
    })?);
    std::fs::write(&staged, bytes)?;
    let dir = path.parent().unwrap_or(path);
    let script = crate::install::elevation_script::write_managed_file_script(dir, &staged, path);
    run(&script, prompt)
}

#[cfg(target_os = "macos")]
fn remove_elevated(path: &Path, prompt: &str) -> io::Result<()> {
    let script = crate::install::elevation_script::remove_managed_file_script(path);
    run(&script, prompt)
}

#[cfg(target_os = "macos")]
fn run(script: &str, prompt: &str) -> io::Result<()> {
    use crate::install::elevate::ElevationError;
    match crate::install::elevate::run_privileged(script, prompt) {
        Ok(()) => Ok(()),
        Err(ElevationError::UserCancelled) => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "administrator approval was declined — the managed configuration was not written",
        )),
        Err(e) => Err(io::Error::other(e.to_string())),
    }
}

#[cfg(target_os = "windows")]
fn write_elevated(path: &Path, bytes: &[u8], _prompt: &str) -> io::Result<()> {
    use crate::install::elevated_job::{ElevatedJob, ManagedFileJob};
    let staging = tempfile::Builder::new()
        .prefix("systemprompt-managed-")
        .tempdir()?;
    let staged = staging.path().join("managed-file");
    std::fs::write(&staged, bytes)?;
    let job = ElevatedJob {
        reg_path: None,
        org_plugins: None,
        clear_values: Vec::new(),
        bridge_values: Vec::new(),
        managed_files: vec![ManagedFileJob {
            staged,
            dest: path.to_path_buf(),
        }],
        remove_files: Vec::new(),
    };
    crate::install::elevated_job::elevate_and_run(staging.path(), &job)
}

#[cfg(target_os = "windows")]
fn remove_elevated(path: &Path, _prompt: &str) -> io::Result<()> {
    use crate::install::elevated_job::ElevatedJob;
    let staging = tempfile::Builder::new()
        .prefix("systemprompt-managed-")
        .tempdir()?;
    let job = ElevatedJob {
        reg_path: None,
        org_plugins: None,
        clear_values: Vec::new(),
        bridge_values: Vec::new(),
        managed_files: Vec::new(),
        remove_files: vec![path.to_path_buf()],
    };
    crate::install::elevated_job::elevate_and_run(staging.path(), &job)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn write_elevated(path: &Path, _bytes: &[u8], _prompt: &str) -> io::Result<()> {
    Err(root_required(path))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn remove_elevated(path: &Path, _prompt: &str) -> io::Result<()> {
    Err(root_required(path))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn root_required(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!(
            "{} is admin-owned and this process cannot write it; re-run as root: sudo {}",
            path.display(),
            std::env::current_exe()
                .ok()
                .as_deref()
                .map_or_else(|| "systemprompt".into(), |p| p.display().to_string()),
        ),
    )
}

#[path = "managed_file_test_api.rs"]
pub mod test_api;
