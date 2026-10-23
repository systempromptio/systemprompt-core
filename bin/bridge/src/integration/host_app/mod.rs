//! `HostApp` trait: per-host credential and profile integration contract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::Serialize;

pub use systemprompt_models::bridge::host::HostAppKind;
use systemprompt_models::bridge::host::HostKind;

mod error;
mod probe;

pub use crate::install::profile_inputs::{
    ModelProtocolOverrides, ProfileFacts, ProfileFactsError, ProfileGenInputs,
};
pub use error::HostAppError;
pub use probe::{DesiredError, ProbeEnv};

pub use crate::integration::profile_state::{
    AppInstallState, ProfileCode, ProfileProbe, ProfileState, StaleReason,
};
use crate::sysproc::{ProcessHit, SysprocError};

/// The outcome of looking for a host's processes: `running` is `None` when
/// the enumeration itself failed, which is not the same as "none found".
#[derive(Debug, Clone, Default)]
pub struct HostProcesses {
    pub running: Option<bool>,
    pub processes: Vec<String>,
    pub error: Option<String>,
}

impl HostProcesses {
    #[must_use]
    pub fn from_enumeration(
        found: Result<Vec<ProcessHit>, SysprocError>,
        packaging: HostAppKind,
    ) -> Self {
        match found {
            Ok(hits) => {
                let processes: Vec<String> = hits
                    .into_iter()
                    .filter(|hit| hit.packaging == packaging)
                    .map(|hit| hit.path)
                    .collect();
                Self {
                    running: Some(!processes.is_empty()),
                    processes,
                    error: None,
                }
            },
            Err(e) => Self {
                running: None,
                processes: Vec::new(),
                error: Some(e.to_string()),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAppSnapshot {
    pub host_id: HostKind,
    pub display_name: &'static str,
    pub profile_state: ProfileState,
    pub profile_source: Option<String>,
    pub profile_keys: BTreeMap<String, String>,
    pub probe_error: Option<String>,
    pub host_running: Option<bool>,
    pub host_processes: Vec<String>,
    pub app_installed: AppInstallState,
    pub probed_at_unix: u64,
    pub update_needs_approval: bool,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct GeneratedProfile {
    pub path: String,
    pub bytes: usize,
    pub payload_uuid: String,
    pub profile_uuid: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum ConfigFormat {
    Json,
    Toml,
    Yaml,
    Plist,
    Reg,
}

/// The outcome of taking a host's systemprompt settings back out.
///
/// `ManualStepRequired` is not a failure: on macOS both hosts are configured by
/// a profile the OS holds on the user's behalf, and only the user can withdraw
/// it. Reporting that as a removal would be a lie, and reporting it as an error
/// would be wrong. `RetainedForSibling` is the shared-config case: the other
/// packaging of the same product still reads the file, so nothing was removed.
#[derive(Debug)]
pub enum ProfileRemoval {
    Removed { path: Option<String> },
    NothingToRemove,
    ManualStepRequired { instruction: String },
    RetainedForSibling { sibling: HostKind },
}

/// Outcome of a successful profile install.
///
/// A warning is a check that could not confirm something the install itself
/// already did (the policy is written, the directory exists) — the host counts
/// as installed and the operator sees the text without a failed step.
#[derive(Debug, Default)]
pub struct ProfileInstalled {
    pub warnings: Vec<String>,
}

impl ProfileInstalled {
    #[must_use]
    pub fn ok() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_warning(warning: String) -> Self {
        Self {
            warnings: vec![warning],
        }
    }
}

pub trait HostApp: Send + Sync + 'static {
    fn id(&self) -> HostKind;
    fn display_name(&self) -> &'static str;
    fn probe(&self, env: &ProbeEnv) -> HostAppSnapshot;

    fn desired(
        &self,
        inputs: &ProfileGenInputs,
    ) -> Result<crate::drift::DesiredProfile, HostAppError>;

    fn app_installed(&self, start_menu: &crate::probe_cache::StartMenuCache) -> AppInstallState;
    fn generate_profile(&self, inputs: &ProfileGenInputs)
    -> Result<GeneratedProfile, HostAppError>;
    fn install_profile(&self, path: &str) -> Result<ProfileInstalled, HostAppError>;
    fn install_action_label(&self) -> &'static str;

    // Why: a repair the bridge starts on its own — at launch, after an upgrade
    // changed what a profile must contain — may not raise an operating-system
    // prompt the user did not ask for. A host whose install needs one answers
    // `HostAppError::NeedsPrompt` here and is left for the user's own Repair.
    fn install_profile_unattended(&self, path: &str) -> Result<ProfileInstalled, HostAppError> {
        self.install_profile(path)
    }

    fn remove_profile(&self) -> Result<ProfileRemoval, HostAppError> {
        Ok(ProfileRemoval::ManualStepRequired {
            instruction: format!(
                "Remove the {} settings from this agent's configuration by hand.",
                crate::brand::brand().binary_name
            ),
        })
    }

    fn open(&self) -> Result<(), HostAppError> {
        Err(HostAppError::OpenUnsupported)
    }

    fn can_open(&self) -> bool {
        true
    }

    // Why: Quit the running app so a relaunch reads the profile just written.
    fn quit(&self) -> Result<(), HostAppError> {
        Err(HostAppError::OpenUnsupported)
    }

    fn can_quit(&self) -> bool {
        false
    }

    fn kind(&self) -> HostAppKind {
        self.id().kind()
    }

    // Why: two packagings of a product read one config tree, so the profile
    // written for either carries one loopback credential and one override
    // key; the host that owns them answers here for both.
    fn profile_identity(&self) -> HostKind {
        self.id()
    }

    fn description(&self) -> &'static str {
        ""
    }

    fn icon_id(&self) -> &'static str {
        self.id().as_str()
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Json
    }

    fn download_url(&self) -> &'static str {
        ""
    }

    // Why: only a host whose profile itself names the managed MCP servers
    // (Claude Desktop's policy) needs the registry refreshed before a
    // generate; every other host reaches them through the synced org-plugins.
    fn profile_carries_managed_servers(&self) -> bool {
        false
    }

    // Why: a failed sync step whose effect the probe compares is superseded
    // by the probe — a later repair that wrote it shows as current, and one
    // still missing shows as drift — so it is not reported a second time.
    fn probe_observes(&self, _emitter: &str, _needs_elevation: bool) -> bool {
        false
    }
}
