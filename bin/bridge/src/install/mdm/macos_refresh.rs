//! The unattended macOS sync step for Claude Desktop's managed preferences.
//!
//! Sync never raises the administrator prompt. A profile installed through
//! System Settings differs from the plists `apply` writes in path and bytes, so
//! the caller judges currency by the probe's rule and hands the judge in: this
//! layer sits below the host integrations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]

use super::{MdmError, MdmPayloadInputs};
use crate::host_sync::{DesktopPolicyJudge, DesktopPolicyState};

pub(crate) fn refresh_unattended(
    mcp: &MdmPayloadInputs<'_>,
    judge: DesktopPolicyJudge,
) -> Result<String, MdmError> {
    mcp.loopback
        .secret_or_mint()
        .map_err(|source| MdmError::Io {
            action: "read loopback secret",
            path: crate::proxy::secret::secret_path().unwrap_or_default(),
            source,
        })?;
    let current = match judge(mcp.loopback, mcp.registry, mcp.overrides) {
        DesktopPolicyState::Current => "Claude Desktop managed preferences are current",
        DesktopPolicyState::Absent => "Claude Desktop managed preferences are not installed",
        DesktopPolicyState::Behind { detail } => {
            return Err(MdmError::ElevationNeeded { detail });
        },
        DesktopPolicyState::Unreadable { reason } => {
            return Err(MdmError::ManagedPrefsUnreadable { reason });
        },
    };
    let standalone =
        super::claude_code_settings::write_standalone_settings(&mcp.loopback.origin())?;
    let mut lines = vec![current.to_owned()];
    lines.extend(standalone.lines);
    Ok(lines.join("; "))
}
