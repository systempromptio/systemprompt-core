//! Provisioning of the org-plugins directory Claude Desktop reads from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SyncError;
use crate::config::paths;

// Why: Claude Desktop on macOS scans org-plugins only under the root-owned
// system directory.
#[cfg(target_os = "macos")]
pub(super) async fn provision_system_org_plugins(
    bridge: &crate::context::BridgeContext,
    path: &std::path::Path,
    operation: std::sync::Arc<tokio::sync::OwnedMutexGuard<()>>,
) -> Result<(), SyncError> {
    let missing = || SyncError::OrgPluginsNeedElevation {
        bin: crate::brand::brand().binary_name,
        system_path: path.display().to_string(),
    };
    if bridge
        .elevation_attempted
        .swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        return Err(missing());
    }
    let user = std::env::var("USER").unwrap_or_default();
    if user.is_empty() || user == "root" {
        return Err(missing());
    }
    let quote = crate::install::elevation_script::shell_quote;
    let script = format!(
        "set -e\nmkdir -p {}\n/usr/sbin/chown -R {} {}\n",
        quote(&path.display().to_string()),
        quote(&user),
        quote(&path.display().to_string())
    );
    tracing::info!(
        path = %path.display(),
        "requesting one-time administrator approval to provision org-plugins for Cowork"
    );
    let outcome = tokio::task::spawn_blocking(move || {
        let _operation = operation;
        crate::install::elevate::run_privileged(
            &script,
            "Bridge needs administrator privileges to create the Claude Desktop org-plugins folder.",
        )
    })
    .await;
    match outcome {
        Ok(Ok(())) if path.is_dir() => {
            crate::fsutil::verify_directory_write(path).map_err(|e| {
                SyncError::Network(format!("verify org-plugins access {}: {e}", path.display()))
            })?;
            tracing::info!(path = %path.display(), "provisioned system org-plugins directory");
            Ok(())
        },
        Ok(Ok(())) => Err(missing()),
        Ok(Err(e)) => Err(SyncError::Network(format!(
            "provision {}: {e}",
            path.display()
        ))),
        Err(e) => Err(SyncError::Network(format!(
            "provisioning task for {}: {e}",
            path.display()
        ))),
    }
}

#[cfg(not(target_os = "macos"))]
#[expect(
    clippy::unused_async,
    reason = "signature must match the macOS variant so run_once stays cfg-free"
)]
pub(super) async fn provision_system_org_plugins(
    _bridge: &crate::context::BridgeContext,
    path: &std::path::Path,
    _operation: std::sync::Arc<tokio::sync::OwnedMutexGuard<()>>,
) -> Result<(), SyncError> {
    Err(SyncError::PathMissing {
        bin: crate::brand::brand().binary_name,
        path: path.display().to_string(),
    })
}

// Why: the root passes the write probe, so only a directory inside it that
// another account created (an upgrade run as Administrator) reports denied;
// the same elevated re-grant that provisions the root also repairs the tree.
#[cfg(target_os = "windows")]
pub(super) fn denied_inside_system_root(
    error: &crate::host_sync::ApplyError,
    location: &paths::OrgPluginsLocation,
) -> bool {
    matches!(
        error,
        crate::host_sync::ApplyError::Io { source, .. }
            if source.kind() == std::io::ErrorKind::PermissionDenied
    ) && location.scope == paths::Scope::System
}

#[cfg(target_os = "windows")]
pub(super) fn org_plugins_denied(
    error: &crate::host_sync::ApplyError,
    location: &paths::OrgPluginsLocation,
) -> SyncError {
    SyncError::Elevation(format!(
        "{error}; the current user cannot replace a plugin directory under {}. Re-run `{} \
         install --apply` and approve the administrator prompt to restore the Modify grant on \
         the whole tree, or remove the directory as an administrator",
        location.path.display(),
        crate::brand::brand().binary_name
    ))
}

#[cfg(not(target_os = "windows"))]
pub(super) const fn denied_inside_system_root(
    _error: &crate::host_sync::ApplyError,
    _location: &paths::OrgPluginsLocation,
) -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub(super) fn org_plugins_denied(
    error: &crate::host_sync::ApplyError,
    location: &paths::OrgPluginsLocation,
) -> SyncError {
    SyncError::Elevation(format!(
        "{error}; the current user cannot replace a plugin directory under {}",
        location.path.display()
    ))
}

#[cfg(not(target_os = "windows"))]
pub(super) async fn heal_org_plugins_scope(
    _bridge: &crate::context::BridgeContext,
    _operation: std::sync::Arc<tokio::sync::OwnedMutexGuard<()>>,
) -> Result<Option<paths::OrgPluginsLocation>, SyncError> {
    Ok(None)
}

#[cfg(target_os = "windows")]
pub(super) async fn heal_org_plugins_scope(
    bridge: &crate::context::BridgeContext,
    operation: std::sync::Arc<tokio::sync::OwnedMutexGuard<()>>,
) -> Result<Option<paths::OrgPluginsLocation>, SyncError> {
    if bridge
        .elevation_attempted
        .swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        return Ok(None);
    }
    let org = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user()
        .map_err(|e| SyncError::Network(format!("org-plugins provisioning: {e}")))?;
    let stage_dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    std::fs::create_dir_all(&stage_dir)
        .map_err(|e| SyncError::Network(format!("create {}: {e}", stage_dir.display())))?;
    tracing::info!(
        path = %org.path.display(),
        "requesting one-time administrator approval to provision org-plugins for Cowork"
    );
    let job = crate::install::elevated_job::ElevatedJob {
        clear_values: Vec::new(),
        bridge_values: Vec::new(),
        managed_files: Vec::new(),
        remove_files: Vec::new(),
        reg_path: None,
        org_plugins: Some(org),
    };
    let outcome = tokio::task::spawn_blocking(move || {
        let _operation = operation;
        crate::install::elevated_job::elevate_and_run(&stage_dir, &job)
    })
    .await;
    let receipt = outcome
        .map_err(|e| SyncError::Network(format!("org-plugins provisioning task: {e}")))?
        .map_err(|e| SyncError::Network(format!("org-plugins provisioning: {e}")))?;
    for step in receipt.steps() {
        bridge
            .activity
            .append(format!("verified {} {}", step.operation, step.target));
    }
    Ok(paths::org_plugins_effective().filter(|l| l.scope == paths::Scope::System))
}
