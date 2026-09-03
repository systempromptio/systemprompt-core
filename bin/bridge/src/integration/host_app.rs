//! `HostApp` trait: per-host credential and profile integration contract.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::Serialize;

use systemprompt_models::services::ApiSurface;

pub use crate::integration::profile_state::{
    AppInstallState, ProfileCode, ProfileState, StaleReason,
};

/// What a host probe needs to know about the proxy to judge a profile fresh:
/// the port the proxy is on and the fingerprint of the secret it accepts.
///
/// A value built by the caller from the [`crate::proxy::LoopbackEndpoint`],
/// so a probe never reaches for process state and a test can hand it any
/// port it likes.
#[derive(Debug, Clone)]
pub struct ProbeEnv {
    pub proxy_port: u16,
    pub loopback_secret_fingerprint: Option<String>,
    pub start_menu: std::sync::Arc<crate::probe_cache::StartMenuCache>,
}

impl ProbeEnv {
    #[must_use]
    pub fn new(
        loopback: &crate::proxy::LoopbackEndpoint,
        start_menu: std::sync::Arc<crate::probe_cache::StartMenuCache>,
    ) -> Self {
        Self {
            proxy_port: loopback.port(),
            loopback_secret_fingerprint: loopback.secret_fingerprint(),
            start_menu,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProfileGenInputs {
    pub gateway_base_url: String,
    pub api_key: String,
    pub models: Vec<String>,
    pub organization_uuid: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub mcp_servers: Vec<crate::install::mdm::policy::McpServerEntry>,
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
    pub host_running: bool,
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

pub trait HostApp: Send + Sync + 'static {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn config_schema(&self) -> &'static HostConfigSchema;
    fn probe(&self, env: &ProbeEnv) -> HostAppSnapshot;
    fn generate_profile(&self, inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile>;
    fn install_profile(&self, path: &str) -> std::io::Result<()>;
    fn install_action_label(&self) -> &'static str;

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

    // Why: a terminal-only CLI has nothing to bring to the foreground, and the
    // verdict must not offer an Open button whose only outcome is an error.
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
}
