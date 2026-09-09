//! Install/uninstall orchestration for the bridge and its scheduled sync task.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod apply;
pub mod bootstrap;
mod builders;
#[cfg(target_os = "macos")]
pub(crate) mod elevate;
#[cfg(target_os = "windows")]
pub(crate) mod elevated_job;
pub mod elevated_protocol;
pub mod elevation_script;
mod error;
pub mod managed_file;
pub mod managed_mcp;
mod managed_profile;
pub mod mdm;
pub mod reg_values;
mod schedule_apply;
mod schedule_emit;
mod summary;
pub(crate) mod xml;

pub use apply::install;
pub use builders::{InstallOptionsBuilder, UninstallSummaryBuilder};
pub use error::InstallError;
pub use mdm::{
    MdmError, MdmPayloadInputs, bridge_policy_values, cowork_egress_allowed_hosts,
    default_inference_models, is_uuid_like, parse_egress_allowed_hosts, snippet as mdm_snippet,
};
pub use schedule_apply::{
    ScheduleStatus, apply_gui_autostart, apply_schedule, gui_autostart_status,
    remove_gui_autostart, remove_schedule, schedule_label, schedule_status,
};
pub use schedule_emit::emit_schedule;
pub use summary::{render_install_summary, render_uninstall_summary};

use crate::config::paths;
use crate::ids::PinnedPubKey;
use crate::schedule::Os;
use crate::stdio::diag;
#[cfg(target_os = "macos")]
pub use mdm::macos::{
    build_bridge_prefs_plist as build_macos_bridge_prefs_plist,
    build_mobileconfig as build_macos_mobileconfig, build_prefs_plist as build_macos_prefs_plist,
};
use std::fs;
use std::path::PathBuf;
use systemprompt_identifiers::ValidatedUrl;

#[derive(Debug)]
pub struct InstallOptions {
    pub print_mdm: Option<Os>,
    pub emit_schedule_template: Option<Os>,
    pub gateway_url: Option<ValidatedUrl>,
    pub pubkey: Option<PinnedPubKey>,
    pub apply: bool,
    pub apply_mobileconfig: bool,
    pub apply_schedule: bool,
    pub egress_allowed_hosts: Option<Vec<String>>,
}

impl InstallOptions {
    #[must_use]
    pub fn builder() -> InstallOptionsBuilder {
        InstallOptionsBuilder::new()
    }
}

#[derive(Debug, Clone)]
pub enum InstallStep {
    Directory(PathBuf),
    Sentinel(PathBuf),
    GatewayConfigured,
    TrustConfigured,
    Policy { outcome: MdmDisplay },
    Schedule { outcome: ScheduleDisplay },
}

#[derive(Debug)]
#[must_use]
pub struct InstallSummary {
    pub location: paths::OrgPluginsLocation,
    pub binary: PathBuf,
    pub mdm: MdmDisplay,
    pub schedule: Option<ScheduleDisplay>,
}

#[derive(Debug, Clone)]
pub enum MdmDisplay {
    Snippet { os: Os, snippet: String },
    Applied { os: Os, report: mdm::MdmApplication },
    MobileconfigPrepared { lines: Vec<String> },
}

/// What the install did about the periodic sync job: wrote a template for the
/// user to install by hand, or registered it with the host scheduler.
#[derive(Debug, Clone)]
pub enum ScheduleDisplay {
    Template(ScheduleEmit),
    Applied(ScheduleApplied),
}

#[derive(Debug, Clone)]
pub struct ScheduleEmit {
    pub os: Os,
    pub path: PathBuf,
    pub install_hint: String,
}

#[derive(Debug, Clone)]
pub struct ScheduleApplied {
    pub os: Os,
    pub label: String,
    pub path: PathBuf,
    pub lines: Vec<String>,
}

#[derive(Debug)]
pub enum ScheduleRemoval {
    NotInstalled(String),
    Removed(String),
    Failed(String),
}

#[derive(Debug)]
pub struct UninstallSummary {
    pub metadata_removed: Option<PathBuf>,
    pub metadata_already_clean: Option<PathBuf>,
    pub managed_profile: ManagedProfileOutcome,
    pub credentials: CredentialsOutcome,
    pub schedule: ScheduleRemoval,
}

impl UninstallSummary {
    #[must_use]
    pub const fn builder() -> UninstallSummaryBuilder {
        UninstallSummaryBuilder::new()
    }
}

#[derive(Debug)]
pub enum ManagedProfileOutcome {
    NotApplicable,
    Removed(&'static str),
    NotInstalled(&'static str),
    RemoveFailed(String),
}

#[derive(Debug)]
pub enum CredentialsOutcome {
    Purged(PathBuf),
    Kept,
}

#[must_use]
pub const fn os_label(os: Os) -> &'static str {
    mdm::os_label(os)
}

#[tracing::instrument(level = "info")]
pub fn uninstall(
    purge: bool,
    bridge: &crate::context::BridgeContext,
) -> Result<UninstallSummary, InstallError> {
    let location = paths::org_plugins_effective().ok_or(InstallError::OrgPluginsUnresolvable)?;

    let metadata = paths::bridge_metadata_dir()
        .ok_or_else(|| InstallError::Bootstrap("bridge metadata dir unresolvable".into()))?;
    let (metadata_removed, metadata_already_clean) = if metadata.exists() {
        fs::remove_dir_all(&metadata).map_err(|e| {
            InstallError::Bootstrap(format!(
                "failed to remove metadata dir {}: {e}",
                metadata.display()
            ))
        })?;
        (Some(metadata), None)
    } else {
        (None, Some(metadata))
    };

    if let Some(staging) = paths::bridge_staging_dir() {
        match fs::remove_dir_all(&staging) {
            Ok(()) => {},
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => {
                return Err(InstallError::Bootstrap(format!(
                    "remove {}: {e}",
                    staging.display()
                )));
            },
        }
    }
    purge_plugin_dirs(&location.path)?;

    let schedule = remove_schedule(&bridge.schedule);
    if let ScheduleRemoval::Failed(e) = &schedule {
        return Err(InstallError::ScheduleApply(e.clone()));
    }

    let managed_profile = managed_profile::remove();
    if let ManagedProfileOutcome::RemoveFailed(e) = &managed_profile {
        return Err(InstallError::Bootstrap(e.clone()));
    }

    let credentials = if purge {
        match crate::auth::setup::logout() {
            Ok(p) => CredentialsOutcome::Purged(p.pat_file),
            Err(e) => {
                let msg = format!("credential purge failed: {e}");
                diag(&msg);
                return Err(InstallError::Bootstrap(msg));
            },
        }
    } else {
        CredentialsOutcome::Kept
    };

    Ok(UninstallSummary {
        metadata_removed,
        metadata_already_clean,
        managed_profile,
        credentials,
        schedule,
    })
}

fn purge_plugin_dirs(root: &std::path::Path) -> Result<(), InstallError> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(InstallError::Bootstrap(format!(
                "enumerate {}: {e}",
                root.display()
            )));
        },
    };
    for entry in entries {
        let entry = entry
            .map_err(|e| InstallError::Bootstrap(format!("enumerate {}: {e}", root.display())))?;
        let kind = entry.file_type().map_err(|e| {
            InstallError::Bootstrap(format!("inspect {}: {e}", entry.path().display()))
        })?;
        if kind.is_dir() && !entry.file_name().to_string_lossy().starts_with('.') {
            fs::remove_dir_all(entry.path()).map_err(|e| {
                InstallError::Bootstrap(format!("remove {}: {e}", entry.path().display()))
            })?;
        }
    }
    Ok(())
}
