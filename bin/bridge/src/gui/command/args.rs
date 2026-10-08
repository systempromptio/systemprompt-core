//! Deserialized argument shapes for GUI IPC commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;

use systemprompt_models::bridge::host::HostKind;

use crate::auth::secret::Secret;

#[derive(Debug, Deserialize)]
pub(super) struct GatewaySetArgs {
    pub(super) url: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct LoginArgs {
    pub(super) token: Secret,
    #[serde(default)]
    pub(super) gateway: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct SessionLoginArgs {
    pub(super) gateway: Option<String>,
    pub(super) keep_signed_in: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct HostIdArgs {
    #[serde(rename = "hostId")]
    pub(super) host_id: HostKind,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct McpProbeArgs {
    #[serde(rename = "serverId")]
    pub(super) server_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct CancelArgs {
    pub(super) scope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct HostInstallArgs {
    #[serde(rename = "hostId")]
    pub(super) host_id: HostKind,
    pub(super) path: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct OpenExternalUrlArgs {
    pub(super) url: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct HostModelFilterArgs {
    #[serde(rename = "hostId")]
    pub(super) host_id: HostKind,
    #[serde(default)]
    pub(super) protocols: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct RecentArgs {
    pub(super) limit: Option<usize>,
}
