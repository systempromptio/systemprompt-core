//! Provisioning of the org-plugins directory Claude Desktop reads from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SyncError;

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
