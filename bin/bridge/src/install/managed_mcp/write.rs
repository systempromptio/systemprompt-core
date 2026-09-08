//! Removing the policy files, escalating to one administrator prompt only when
//! the direct removal is refused for permissions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::claude_policy::MANAGED_SETTINGS_FILE;

pub(super) fn clear_direct(
    mcp_path: &Path,
    settings_path: &Path,
    stripped_settings_body: Option<&str>,
) -> std::io::Result<()> {
    match fs::remove_file(mcp_path) {
        Ok(()) => {},
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => return Err(e),
    }
    if mcp_path.try_exists()? {
        return Err(std::io::Error::other(format!(
            "{} still exists after removal",
            mcp_path.display()
        )));
    }
    if let Some(body) = stripped_settings_body {
        crate::fsutil::atomic_write_0644(settings_path, body.as_bytes())?;
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn stage_clear(body: &str) -> Result<(tempfile::TempDir, std::path::PathBuf), std::io::Error> {
    let dir = tempfile::Builder::new().prefix("bridge-clear-").tempdir()?;
    let staged = dir.path().join(MANAGED_SETTINGS_FILE);
    fs::write(&staged, body.as_bytes())?;
    Ok((dir, staged))
}

#[cfg(target_os = "macos")]
pub(super) fn clear_elevated(
    mcp_path: &Path,
    settings_path: &Path,
    stripped_settings_body: Option<&str>,
) -> std::io::Result<()> {
    let staging = stripped_settings_body.map(stage_clear).transpose()?;
    let script = crate::install::elevation_script::clear_policy_script(
        mcp_path.exists().then_some(mcp_path),
        staging
            .as_ref()
            .map(|(_, staged)| (staged.as_path(), settings_path)),
    );
    crate::install::elevate::run_privileged(
        &script,
        "Bridge needs administrator privileges to remove the Claude Code enterprise MCP policy.",
    )
    .map_err(std::io::Error::other)?;
    verify_removal(mcp_path, settings_path, stripped_settings_body)
}

#[cfg(target_os = "windows")]
pub(super) fn clear_elevated(
    mcp_path: &Path,
    settings_path: &Path,
    stripped_settings_body: Option<&str>,
) -> std::io::Result<()> {
    let staging = stripped_settings_body.map(stage_clear).transpose()?;
    let job = crate::install::elevated_job::ElevatedJob {
        reg_path: None,
        org_plugins: None,
        clear_values: Vec::new(),
        bridge_values: Vec::new(),
        managed_files: staging
            .iter()
            .map(|(_, staged)| crate::install::elevated_job::ManagedFileJob {
                staged: staged.clone(),
                dest: settings_path.to_path_buf(),
            })
            .collect(),
        remove_files: if mcp_path.exists() {
            vec![mcp_path.to_path_buf()]
        } else {
            Vec::new()
        },
    };
    let stage_dir = staging
        .as_ref()
        .map_or_else(std::env::temp_dir, |(dir, _)| dir.path().to_path_buf());
    let receipt = crate::install::elevated_job::elevate_and_run(&stage_dir, &job)?;
    for file in &job.managed_files {
        receipt.require("install", &file.dest)?;
    }
    for path in &job.remove_files {
        receipt.require("remove", path)?;
    }
    verify_removal(mcp_path, settings_path, stripped_settings_body)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(super) fn clear_elevated(mcp_path: &Path, _: &Path, _: Option<&str>) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        format!(
            "{}: root privileges required to remove managed MCP policy",
            mcp_path.display()
        ),
    ))
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn verify_removal(
    mcp_path: &Path,
    settings_path: &Path,
    body: Option<&str>,
) -> std::io::Result<()> {
    if mcp_path.try_exists()? {
        return Err(std::io::Error::other(format!(
            "{}: elevated removal did not land",
            mcp_path.display()
        )));
    }
    if let Some(body) = body {
        crate::fsutil::verify_contents(settings_path, body.as_bytes())?;
    }
    Ok(())
}
