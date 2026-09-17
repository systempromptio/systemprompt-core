//! Refuses a sync that would put org plugins where the enabled hosts cannot
//! read them without elevation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SyncError;
use crate::config::paths;
use crate::gateway::manifest::SignedManifest;

#[cfg(target_os = "windows")]
pub(super) fn check_org_plugins_scope(
    manifest: &SignedManifest,
    location: &paths::OrgPluginsLocation,
) -> Result<(), SyncError> {
    if manifest.enabled_hosts.iter().any(|h| h == "claude-desktop")
        && let paths::FallbackReason::SystemUnwritable { system_path } = &location.reason
    {
        return Err(SyncError::OrgPluginsNeedElevation {
            bin: crate::brand::brand().binary_name,
            system_path: system_path.display().to_string(),
        });
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "signature must match the windows variant so run_once stays cfg-free"
)]
pub(super) const fn check_org_plugins_scope(
    _manifest: &SignedManifest,
    _location: &paths::OrgPluginsLocation,
) -> Result<(), SyncError> {
    Ok(())
}
