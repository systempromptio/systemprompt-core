//! Writing and removing the policy files, escalating to an elevated shell only
//! when the direct write is refused for permissions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;
#[cfg(target_os = "macos")]
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use super::{MANAGED_MCP_FILE, MANAGED_SETTINGS_FILE};

pub(super) fn write_policy_file(path: &Path, body: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, body.as_bytes())
}

// Why: a read error counts as "does not match" so an unreadable file still
// triggers the write path — idempotent syncs must skip elevation, but an
// unknown on-disk state must never be mistaken for an up-to-date one.
pub(super) fn body_matches(path: &Path, body: &str) -> bool {
    fs::read(path).is_ok_and(|bytes| bytes == body.as_bytes())
}

pub(super) enum WriteOutcome {
    Ok,
    #[cfg_attr(
        not(target_os = "macos"),
        expect(
            dead_code,
            reason = "only the macOS elevation path can be declined by the operator"
        )
    )]
    Declined,
    Failed(String),
}

pub(super) fn write_both(
    mcp_path: &Path,
    mcp_body: &str,
    settings_path: &Path,
    settings_body: &str,
) -> WriteOutcome {
    // Why: try the direct write first — CI, root shells and MDM-provisioned
    // users are already privileged and must not be prompted at all.
    let direct = write_policy_file(mcp_path, mcp_body)
        .and_then(|()| write_policy_file(settings_path, settings_body));
    match direct {
        Ok(()) => WriteOutcome::Ok,
        // Why: only permission-denied justifies escalating — anything else
        // (ENOSPC and friends) is a real failure and elevation cannot fix it.
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            write_elevated(mcp_path, mcp_body, settings_path, settings_body)
        },
        Err(err) => WriteOutcome::Failed(err.to_string()),
    }
}

#[cfg(target_os = "macos")]
fn write_elevated(
    mcp_path: &Path,
    mcp_body: &str,
    settings_path: &Path,
    settings_body: &str,
) -> WriteOutcome {
    // Why: stage into a user-writable tempdir first — the elevated shell can
    // read it, whereas a heredoc would embed the body in the script itself.
    let staging = match stage_temp(mcp_body, settings_body) {
        Ok(t) => t,
        Err(e) => return WriteOutcome::Failed(format!("stage temp: {e}")),
    };
    let script = crate::install::elevation_script::write_policy_script(
        mcp_path.parent().unwrap_or(mcp_path),
        &staging.mcp,
        mcp_path,
        &staging.settings,
        settings_path,
    );
    let result = crate::install::elevate::run_privileged(
        &script,
        "Astound Bridge needs administrator privileges to install the Claude Code enterprise MCP policy.",
    );
    match result {
        Ok(()) => WriteOutcome::Ok,
        Err(crate::install::elevate::ElevationError::UserCancelled) => WriteOutcome::Declined,
        Err(e) => WriteOutcome::Failed(e.to_string()),
    }
}

#[cfg(not(target_os = "macos"))]
fn write_elevated(_: &Path, _: &str, _: &Path, _: &str) -> WriteOutcome {
    WriteOutcome::Failed(
        "administrator privileges required to write the policy directory".to_owned(),
    )
}

#[cfg(target_os = "macos")]
struct TempStaging {
    _dir: tempfile::TempDir,
    mcp: PathBuf,
    settings: PathBuf,
}

#[cfg(target_os = "macos")]
fn stage_temp(mcp_body: &str, settings_body: &str) -> Result<TempStaging, std::io::Error> {
    let dir = tempfile::Builder::new()
        .prefix("astound-install-")
        .tempdir()?;
    let mcp = dir.path().join(MANAGED_MCP_FILE);
    let settings = dir.path().join(MANAGED_SETTINGS_FILE);
    fs::write(&mcp, mcp_body.as_bytes())?;
    fs::write(&settings, settings_body.as_bytes())?;
    Ok(TempStaging {
        _dir: dir,
        mcp,
        settings,
    })
}

pub(super) fn clear_direct(
    mcp_path: &Path,
    settings_path: &Path,
    stripped_settings_body: Option<&str>,
) -> bool {
    let mcp_ok = match fs::remove_file(mcp_path) {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => false,
        Err(_) => false,
    };
    let settings_ok =
        stripped_settings_body.is_none_or(|body| write_policy_file(settings_path, body).is_ok());
    mcp_ok && settings_ok
}

#[cfg(target_os = "macos")]
fn stage_clear(body: &str) -> Result<(tempfile::TempDir, PathBuf), std::io::Error> {
    let dir = tempfile::Builder::new()
        .prefix("astound-clear-")
        .tempdir()?;
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
        "Astound Bridge needs administrator privileges to remove the Claude Code enterprise MCP policy.",
    ) {
        Ok(()) => tracing::info!(
            target: "bridge::install::managed-mcp",
            "Claude Code MCP policy removed"
        ),
        Err(crate::install::elevate::ElevationError::UserCancelled) => tracing::warn!(
            target: "bridge::install::managed-mcp",
            "user declined the administrator authorization prompt; Claude Code MCP policy \
             files were left in place"
        ),
        Err(e) => tracing::warn!(
            target: "bridge::install::managed-mcp",
            error = %e,
            "failed to remove Claude Code MCP policy"
        ),
    }
}
