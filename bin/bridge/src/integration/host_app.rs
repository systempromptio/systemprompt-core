//! `HostApp` trait: per-host credential and profile integration contract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::Serialize;

use systemprompt_models::services::ApiSurface;

use crate::ids::{HostId, HostToken, LoopbackSecret};
pub use crate::integration::profile_state::{
    AppInstallState, Freshness, ProfileCode, ProfileProbe, ProfileState, StaleReason,
};
use crate::sysproc::SysprocError;

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
    pub fn from_enumeration(found: Result<Vec<String>, SysprocError>) -> Self {
        match found {
            Ok(processes) => Self {
                running: Some(!processes.is_empty()),
                processes,
                error: None,
            },
            Err(e) => Self {
                running: None,
                processes: Vec::new(),
                error: Some(e.to_string()),
            },
        }
    }
}


/// What a host probe needs to know about the proxy to judge a profile fresh:
/// the port the proxy is on and the secret it accepts, from which the
/// per-host token a desktop policy carries is derived.
///
/// A value built by the caller from the [`crate::proxy::LoopbackEndpoint`],
/// so a probe never reaches for process state and a test can hand it any
/// port it likes.
#[derive(Debug, Clone)]
pub struct ProbeEnv {
    pub proxy_port: u16,
    pub loopback_secret: Option<LoopbackSecret>,
    pub start_menu: std::sync::Arc<crate::probe_cache::StartMenuCache>,
    pub expected_managed_servers: Option<Vec<String>>,
}

impl ProbeEnv {
    #[must_use]
    pub fn new(
        loopback: &crate::proxy::LoopbackEndpoint,
        start_menu: std::sync::Arc<crate::probe_cache::StartMenuCache>,
    ) -> Self {
        let loopback_secret = match loopback.secret() {
            Ok(secret) => Some(secret),
            Err(error) => {
                tracing::warn!(error = %error, "loopback secret is unreadable; host probes report it as unverifiable");
                None
            },
        };
        Self {
            proxy_port: loopback.port(),
            loopback_secret,
            start_menu,
            expected_managed_servers: None,
        }
    }

    #[must_use]
    pub fn for_bridge(bridge: &crate::context::BridgeContext) -> Self {
        Self::new(
            bridge.proxy.loopback(),
            std::sync::Arc::clone(&bridge.start_menu),
        )
        .with_managed_servers(bridge.proxy.loopback(), &bridge.mcp_registry())
    }

    // Why: the registry is authoritative only once a sync has published it
    // (the on-disk fragment exists); before that an empty registry means
    // "not loaded", not "no servers". A catalog that cannot expand a wildcard
    // tool policy withholds the whole list, and so does this.
    #[must_use]
    pub fn with_managed_servers(
        mut self,
        loopback: &crate::proxy::LoopbackEndpoint,
        registry: &crate::mcp_registry::McpRegistry,
    ) -> Self {
        if !crate::mcp_registry::fragment_exists() {
            return self;
        }
        self.expected_managed_servers = match crate::install::mdm::policy::mcp_entries(loopback, registry) {
            Ok(entries) => entries.map(|list| list.into_iter().map(|e| e.name).collect()),
            Err(error) => {
                tracing::warn!(error = %error, "managed MCP servers could not be projected; the policy probe leaves them unchecked");
                None
            },
        };
        self
    }

    #[must_use]
    pub fn loopback_secret_fingerprint(&self) -> Option<String> {
        self.loopback_secret
            .as_ref()
            .map(|s| crate::proxy::secret::fingerprint(s.as_str()))
    }

    #[must_use]
    pub fn host_token_fingerprint(&self, host: &HostId) -> Option<String> {
        self.loopback_secret.as_ref().map(|s| {
            crate::proxy::secret::fingerprint(
                crate::proxy::scoped_token::host_token(s, host).as_str(),
            )
        })
    }
}

/// Inputs a host renders its profile from.
///
/// `default_model` is the gateway's configured default only when it is one
/// of `models`. `host_token` is the [`crate::ids::HostToken`] derived for
/// this host from the loopback secret; no renderer ever sees the secret
/// itself, so every provisioned host presents a credential the proxy can
/// attribute to it.
#[derive(Debug, Clone)]
pub struct ProfileGenInputs {
    pub gateway_base_url: String,
    pub host_token: HostToken,
    pub models: Vec<String>,
    pub default_model: Option<String>,
    pub organization_uuid: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub mcp_servers: Option<Vec<crate::install::mdm::policy::McpServerEntry>>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct HostConfigSchema {
    pub required_keys: &'static [&'static str],
    pub display_keys: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize)]
pub struct HostAppSnapshot {
    pub host_id: &'static str,
    pub display_name: &'static str,
    pub profile_state: ProfileState,
    pub profile_source: Option<String>,
    pub profile_keys: BTreeMap<String, String>,
    pub probe_error: Option<String>,
    pub host_running: Option<bool>,
    pub host_processes: Vec<String>,
    pub app_installed: AppInstallState,
    pub probed_at_unix: u64,
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
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum HostKind {
    DesktopApp,
    CliTool,
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
/// would be wrong.
#[derive(Debug)]
pub enum ProfileRemoval {
    Removed { path: Option<String> },
    NothingToRemove,
    ManualStepRequired { instruction: String },
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
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn config_schema(&self) -> &'static HostConfigSchema;
    fn probe(&self, env: &ProbeEnv) -> HostAppSnapshot;
    fn generate_profile(&self, inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile>;
    fn install_profile(&self, path: &str) -> std::io::Result<ProfileInstalled>;
    fn install_action_label(&self) -> &'static str;

    // Why: a repair the bridge starts on its own — at launch, after an upgrade
    // changed what a profile must contain — may not raise an operating-system
    // prompt the user did not ask for. A host whose install needs one answers
    // `PermissionDenied` here and is left for the user's own Repair.
    fn install_profile_unattended(&self, path: &str) -> std::io::Result<ProfileInstalled> {
        self.install_profile(path)
    }

    fn remove_profile(&self) -> std::io::Result<ProfileRemoval> {
        Ok(ProfileRemoval::ManualStepRequired {
            instruction: format!(
                "Remove the {} settings from this agent's configuration by hand.",
                crate::brand::brand().binary_name
            ),
        })
    }

    fn open(&self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "open not implemented",
        ))
    }

    fn can_open(&self) -> bool {
        true
    }

    fn kind(&self) -> HostKind {
        HostKind::DesktopApp
    }

    fn description(&self) -> &'static str {
        ""
    }

    fn icon_id(&self) -> &'static str {
        self.id()
    }

    fn config_format(&self) -> ConfigFormat {
        ConfigFormat::Json
    }

    fn download_url(&self) -> &'static str {
        ""
    }

    fn accepted_surfaces(&self) -> &'static [ApiSurface] {
        &[]
    }

    // Why: only a host whose profile itself names the managed MCP servers
    // (Claude Desktop's policy) needs the registry refreshed before a
    // generate; every other host reaches them through the synced org-plugins.
    fn profile_carries_managed_servers(&self) -> bool {
        false
    }

    // Why: whether rewriting this host's installed profile will raise the
    // operating system's administrator prompt — the machine policy hive on
    // Windows, the managed-preferences write on macOS. The verb the GUI
    // offers says so before the user presses it.
    fn update_needs_approval(&self, _snapshot: &HostAppSnapshot) -> bool {
        false
    }
}
