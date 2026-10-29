//! The Claude Desktop machine-policy write that needs administrator approval:
//! the elevated policy writer first, then one UAC prompt through the shared
//! elevation gate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::writer::install_through_writer;
use super::{ApprovalRefusal, Attendance, HostAppError, ProfileInstalled};

#[derive(Clone, Copy)]
pub(super) struct Approver<'a> {
    pub(super) attendance: Attendance,
    pub(super) gate: &'a crate::elevation_gate::ElevationGate,
}

pub(super) fn install_needing_approval(
    path: &str,
    entries: &[(String, String)],
    approver: Approver<'_>,
    reason: &'static str,
    differing: &[String],
) -> Result<ProfileInstalled, HostAppError> {
    let Approver { attendance, gate } = approver;
    match install_through_writer(entries) {
        Ok(Some(installed)) => return Ok(installed),
        Ok(None) => {},
        Err(e) => tracing::warn!(
            path,
            error = %e,
            "the elevated policy writer did not apply the profile; asking for approval"
        ),
    }
    if attendance == Attendance::Unattended {
        tracing::warn!(path, differing = ?differing, reason, "an unattended repair cannot elevate; left for one approval");
        return Err(ApprovalRefusal::NeedsPrompt { reason }.into());
    }
    tracing::warn!(path, differing = ?differing, reason, "requesting administrator approval");
    install_profile_elevated(path, gate)
}

fn install_profile_elevated(
    path: &str,
    gate: &crate::elevation_gate::ElevationGate,
) -> Result<ProfileInstalled, HostAppError> {
    let org = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user()?;
    let stage_dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    std::fs::create_dir_all(&stage_dir)?;
    let job = crate::install::elevated_job::ElevatedJob {
        reg_path: Some(path.to_owned()),
        org_plugins: Some(org),
        clear_values: Vec::new(),
        bridge_values: Vec::new(),
        managed_files: Vec::new(),
        remove_files: Vec::new(),
        private_dirs: Vec::new(),
    };
    let receipt = crate::install::elevated_job::elevate_and_run(gate, &stage_dir, &job)?;
    receipt.require("policy", std::path::Path::new(path))?;
    tracing::info!(
        path,
        "Claude Desktop profile installed through an elevated write"
    );
    Ok(ProfileInstalled::ok())
}
