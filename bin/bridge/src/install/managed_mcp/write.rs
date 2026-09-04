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
) -> bool {
    let mcp_ok = match fs::remove_file(mcp_path) {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(_) => false,
    };
    let settings_ok =
        stripped_settings_body.is_none_or(|body| fs::write(settings_path, body).is_ok());
    mcp_ok && settings_ok
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
) {
    // Why: the staging dir must outlive `run_privileged` — the elevated shell
    // reads the staged file from it.
    let staging = match stripped_settings_body.map(stage_clear).transpose() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                target: "bridge::install::managed-mcp",
                error = %e,
                "could not stage the stripped managed-settings.json for elevation",
            );
            return;
        },
    };
    let script = crate::install::elevation_script::clear_policy_script(
        mcp_path.exists().then_some(mcp_path),
        staging
            .as_ref()
            .map(|(_, staged)| (staged.as_path(), settings_path)),
    );
    match crate::install::elevate::run_privileged(
        &script,
        "Bridge needs administrator privileges to remove the Claude Code enterprise MCP policy.",
    ) {
        Ok(()) => tracing::info!(
            target: "bridge::install::managed-mcp",
            "Claude Code MCP policy removed"
        ),
        Err(crate::install::elevate::ElevationError::UserCancelled) => tracing::warn!(
            target: "bridge::install::managed-mcp",
            "user declined the administrator authorization prompt; the Claude Code MCP policy \
             still shadows plugin servers and Cowork tools"
        ),
        Err(e) => tracing::warn!(
            target: "bridge::install::managed-mcp",
            error = %e,
            "could not remove the Claude Code MCP policy"
        ),
    }
}

#[cfg(target_os = "windows")]
pub(super) fn clear_elevated(
    mcp_path: &Path,
    settings_path: &Path,
    stripped_settings_body: Option<&str>,
) {
    let staging = match stripped_settings_body.map(stage_clear).transpose() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                target: "bridge::install::managed-mcp",
                error = %e,
                "could not stage the stripped managed-settings.json for elevation",
            );
            return;
        },
    };
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
    match crate::install::elevated_job::elevate_and_run(&stage_dir, &job) {
        Ok(()) => tracing::info!(
            target: "bridge::install::managed-mcp",
            "Claude Code MCP policy removed"
        ),
        Err(e) => tracing::warn!(
            target: "bridge::install::managed-mcp",
            error = %e,
            "could not remove the Claude Code MCP policy; it still shadows plugin servers and \
             Cowork tools"
        ),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(super) fn clear_elevated(mcp_path: &Path, _: &Path, _: Option<&str>) {
    tracing::warn!(
        target: "bridge::install::managed-mcp",
        path = %mcp_path.display(),
        "could not remove the Claude Code MCP policy — root privileges required"
    );
}
