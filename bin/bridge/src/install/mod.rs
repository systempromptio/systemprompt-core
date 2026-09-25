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
pub mod elevated_job;
pub mod elevated_protocol;
pub mod elevation_script;
mod error;
pub mod managed_file;
pub mod managed_mcp;
mod managed_profile;
pub mod mdm;
pub mod policy_writer;
pub mod reg_values;
mod schedule_apply;
mod schedule_emit;
mod summary;
pub(crate) mod xml;

pub use apply::install;
pub use builders::InstallOptionsBuilder;
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
    pub foreign_plugins: Vec<String>,
    pub metadata_removed: Option<PathBuf>,
    pub metadata_already_clean: Option<PathBuf>,
    pub managed_profile: ManagedProfileOutcome,
    pub credentials: CredentialsOutcome,
    pub schedule: ScheduleRemoval,
    pub host_warnings: Vec<String>,
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
    #[cfg(target_os = "windows")]
    mdm::claude_code_settings::remove_all().map_err(InstallError::MdmRemove)?;

    let metadata = paths::bridge_metadata_dir().ok_or(InstallError::MetadataUnresolvable)?;
    // Why: the sentinel names the plugin directories the bridge wrote; it is
    // read before the metadata directory that holds it is removed.
    let owned_plugins = crate::last_sync::read_last_sync(&metadata.join(paths::LAST_SYNC_SENTINEL))
        .map_err(InstallError::LastSync)?
        .map(|state| state.present_plugins)
        .unwrap_or_default();
    let (metadata_removed, metadata_already_clean) = if metadata.exists() {
        fs::remove_dir_all(&metadata).map_err(|source| InstallError::Remove {
            path: metadata.clone(),
            source,
        })?;
        (Some(metadata), None)
    } else {
        (None, Some(metadata))
    };

    for dir in [paths::bridge_staging_dir(), paths::bridge_update_dir()]
        .into_iter()
        .flatten()
    {
        match fs::remove_dir_all(&dir) {
            Ok(()) => {},
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(source) => return Err(InstallError::Remove { path: dir, source }),
        }
    }
    let foreign_plugins = purge_plugin_dirs(&location.path, &owned_plugins)?;

    let schedule = remove_schedule(&bridge.schedule);
    if let ScheduleRemoval::Failed(e) = &schedule {
        return Err(InstallError::ScheduleApply(e.clone()));
    }

    let managed_profile = managed_profile::remove(&bridge.policy_store);
    if let ManagedProfileOutcome::RemoveFailed(e) = &managed_profile {
        return Err(InstallError::ManagedProfileRemove(e.clone()));
    }

    let credentials = if purge {
        match crate::auth::setup::logout() {
            Ok(p) => CredentialsOutcome::Purged(p.pat_file),
            Err(e) => return Err(InstallError::CredentialPurge(e)),
        }
    } else {
        CredentialsOutcome::Kept
    };

    Ok(UninstallSummary {
        foreign_plugins,
        metadata_removed,
        metadata_already_clean,
        managed_profile,
        credentials,
        schedule,
        host_warnings: Vec::new(),
    })
}

// Why: only the plugin directories the last sync recorded are the bridge's to
// remove; anything else under org-plugins was put there by someone else and
// is reported, not deleted.
fn purge_plugin_dirs(
    root: &std::path::Path,
    owned: &[String],
) -> Result<Vec<String>, InstallError> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(InstallError::Enumerate {
                path: root.to_path_buf(),
                source,
            });
        },
    };
    let mut foreign = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| InstallError::Enumerate {
            path: root.to_path_buf(),
            source,
        })?;
        let kind = entry
            .file_type()
            .map_err(|source| InstallError::Enumerate {
                path: entry.path(),
                source,
            })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !kind.is_dir() || name.starts_with('.') {
            continue;
        }
        if owned.contains(&name) {
            fs::remove_dir_all(entry.path()).map_err(|source| InstallError::Remove {
                path: entry.path(),
                source,
            })?;
        } else {
            foreign.push(name);
        }
    }
    foreign.sort();
    Ok(foreign)
}
