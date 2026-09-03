//! Provisioning of the org-plugins directory Claude Desktop reads from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{SyncError, paths};

// Why: Claude Desktop only reads org-plugins from a root-owned system path on
// macOS, and the GUI had no way to create it — sync failed closed telling a
// double-click user to run `sudo … install --apply`, which also left the MCP
// registry empty. This raises the same single administrator prompt Windows
// raises, once per process so a declined prompt does not re-fire from auto-sync
// or a watch loop, and hands the directory to the invoking user so later
// unelevated syncs can write it.
#[cfg(target_os = "macos")]
pub(super) async fn provision_system_org_plugins(
    bridge: &crate::context::BridgeContext,
    path: &std::path::Path,
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
    let quoted = path.display().to_string().replace('"', "\\\"");
    let script = format!(
        "set -e\nmkdir -p \"{quoted}\"\n/usr/sbin/chown -R \"{user}\" \"{quoted}\"\n"
    );
    tracing::info!(
        path = %path.display(),
        "requesting one-time administrator approval to provision org-plugins for Cowork"
    );
    let outcome = tokio::task::spawn_blocking(move || {
        crate::install::elevate::run_privileged(
            &script,
            "Bridge needs administrator privileges to create the Claude Desktop org-plugins folder.",
        )
    })
    .await;
    match outcome {
        Ok(Ok(())) if path.is_dir() => {
            tracing::info!(path = %path.display(), "provisioned system org-plugins directory");
            Ok(())
        },
        Ok(Ok(())) => Err(missing()),
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "org-plugins provisioning was not completed");
            Err(missing())
        },
        Err(e) => {
            tracing::warn!(error = %e, "org-plugins provisioning task failed");
            Err(missing())
        },
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
) -> Result<(), SyncError> {
    Err(SyncError::PathMissing {
        bin: crate::brand::brand().binary_name,
        path: path.display().to_string(),
    })
}
