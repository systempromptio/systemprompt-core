//! Install/uninstall orchestration for the bridge and its scheduled sync task.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod apply;
mod bootstrap;
mod builders;
mod error;
pub(crate) mod mdm;
mod schedule_apply;
mod schedule_emit;
mod summary;
#[cfg(target_os = "macos")]
pub(crate) mod xml;

pub use apply::install;
pub use builders::{InstallOptionsBuilder, UninstallSummaryBuilder};
pub use error::InstallError;
#[cfg(target_os = "windows")]
pub use mdm::windows_policy_values;
pub use mdm::{is_uuid_like, snippet as mdm_snippet};
pub use schedule_apply::{apply_schedule, remove_schedule};
pub use schedule_emit::emit_schedule;
pub use summary::{render_install_summary, render_uninstall_summary};

use crate::config::paths;
use crate::ids::PinnedPubKey;
use crate::obs::output::diag;
use crate::schedule::Os;
#[cfg(target_os = "macos")]
pub use mdm::macos::{
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
}

impl InstallOptions {
    #[must_use]
    pub fn builder() -> InstallOptionsBuilder {
        InstallOptionsBuilder::new()
    }
}

#[derive(Debug)]
pub struct InstallSummary {
    pub location: paths::OrgPluginsLocation,
    pub binary: PathBuf,
    pub mdm: MdmDisplay,
    pub schedule: Option<ScheduleDisplay>,
}

#[derive(Debug)]
pub enum MdmDisplay {
    Snippet { os: Os, snippet: String },
    Applied { os: Os, lines: Vec<String> },
    MobileconfigApplied { lines: Vec<String> },
}

/// What the install did about the periodic sync job: wrote a template for the
/// user to install by hand, or registered it with the host scheduler.
#[derive(Debug)]
pub enum ScheduleDisplay {
    Template(ScheduleEmit),
    Applied(ScheduleApplied),
}

#[derive(Debug)]
pub struct ScheduleEmit {
    pub os: Os,
    pub path: PathBuf,
    pub install_hint: String,
}

#[derive(Debug)]
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
    PurgeFailed(String),
}

#[must_use]
pub const fn os_label(os: Os) -> &'static str {
    mdm::os_label(os)
}

#[tracing::instrument(level = "info")]
pub fn uninstall(purge: bool) -> Result<UninstallSummary, InstallError> {
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

    if let Some(staging) = paths::bridge_staging_dir()
        && staging.exists()
    {
        _ = fs::remove_dir_all(&staging);
    }

    if let Ok(entries) = fs::read_dir(&location.path) {
        for entry in entries.flatten() {
            let is_plugin_dir = entry.file_type().is_ok_and(|t| t.is_dir())
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|n| !n.starts_with('.'));
            if is_plugin_dir {
                _ = fs::remove_dir_all(entry.path());
            }
        }
    }

    if let Some(target) = crate::integration::cowork_plugins::resolve_target()
        && let Err(e) = crate::integration::cowork_plugins::clear_all(&target)
    {
        diag(&format!("warning: Cowork enable-key cleanup failed: {e}"));
    }

    let schedule = remove_schedule();
    if let ScheduleRemoval::Failed(e) = &schedule {
        diag(&format!("warning: scheduled sync job removal failed: {e}"));
    }

    let managed_profile = remove_managed_profile();

    let credentials = if purge {
        match crate::auth::setup::logout() {
            Ok(p) => CredentialsOutcome::Purged(p.pat_file),
            Err(e) => {
                let msg = format!("credential purge failed: {e}");
                diag(&msg);
                CredentialsOutcome::PurgeFailed(msg)
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

#[cfg(target_os = "macos")]
fn remove_managed_profile() -> ManagedProfileOutcome {
    match mdm::macos::remove_profile() {
        Ok(true) => ManagedProfileOutcome::Removed(mdm::macos::PAYLOAD_IDENTIFIER),
        Ok(false) => ManagedProfileOutcome::NotInstalled(mdm::macos::PAYLOAD_IDENTIFIER),
        Err(e) => {
            let msg = format!("profile remove failed: {e}");
            diag(&msg);
            ManagedProfileOutcome::RemoveFailed(msg)
        },
    }
}

#[cfg(target_os = "windows")]
fn remove_managed_profile() -> ManagedProfileOutcome {
    match mdm::remove_windows_policy() {
        Ok(true) => {
            ManagedProfileOutcome::Removed("HKCU Policies\\Claude (+ HKLM managedMcpServers)")
        },
        Ok(false) => ManagedProfileOutcome::NotInstalled("Windows Policies\\Claude"),
        Err(e) => {
            diag(&e);
            ManagedProfileOutcome::RemoveFailed(e)
        },
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const fn remove_managed_profile() -> ManagedProfileOutcome {
    ManagedProfileOutcome::NotApplicable
}
