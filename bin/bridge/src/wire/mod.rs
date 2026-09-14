//! GUI payload types exported by `just bridge-bindings`.
//!
//! `bridge-bindings-check` verifies the generated TypeScript definitions.
//! The module is independent of platform GUI libraries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod codes;
pub mod first_run;
pub mod hosts;
pub mod ipc;
pub mod payloads;
mod semantic;

use serde::Serialize;

use crate::verdict::{Tone, Verdict};
use codes::{HealthCode, IdentityCode, OverallCode, TokenCode};
use payloads::{
    CachedTokenPayload, GatewayStatusPayload, McpServerAuthPayload, ProxyStatsPayload,
    StartupFaultPayload, UpdatePayload, ValidationPayload, VerifiedIdentityPayload,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum DeviceAction {
    Disconnect,
    Purge,
    RemoveApplication,
}

/// The whole state snapshot as the webview receives it on `state.snapshot`
/// and every `state.changed`.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
#[expect(
    clippy::struct_excessive_bools,
    reason = "flat JSON wire contract; each flag is a serialised field the GUI reads by name"
)]
pub struct StatePayload<'a> {
    pub gateway_url: &'a str,
    pub gateway_configured: bool,
    pub config_file: &'a str,
    pub pat_file: &'a str,
    pub config_present: bool,
    pub pat_present: bool,
    pub plugins_dir: Option<&'a str>,
    pub last_sync_summary: Option<&'a str>,
    pub last_sync_report: Option<&'a crate::sync::SyncSummary>,
    pub skill_count: Option<usize>,
    pub agent_count: Option<usize>,
    pub plugin_count: Option<usize>,
    pub malformed_plugin_count: Option<usize>,
    pub last_validation: Option<ValidationPayload<'a>>,
    pub last_validation_at_unix: Option<u64>,
    pub health: Verdict<HealthCode>,
    #[cfg_attr(
        feature = "ts-export",
        ts(
            type = "Array<{ name: string; surface: string; configured: boolean; models: \
                    string[]; config_issue?: string }>"
        )
    )]
    pub provider_health: &'a [systemprompt_models::bridge::profile::ProviderHealth],
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub credential_error: Option<&'a str>,
    pub startup_faults: Vec<StartupFaultPayload<'a>>,
    pub sync_in_flight: bool,
    pub cached_token: Option<CachedTokenPayload>,
    pub token: Verdict<TokenCode>,
    pub gateway_status: GatewayStatusPayload<'a>,
    pub verified_identity: Option<VerifiedIdentityPayload<'a>>,
    pub identity: Verdict<IdentityCode>,
    pub cloud_tone: Tone,
    pub overall: Verdict<OverallCode>,
    pub signed_in: bool,
    pub last_probe_at_unix: Option<u64>,
    pub proxy_stats: ProxyStatsPayload,
    pub mcp_auth: Vec<McpServerAuthPayload<'a>>,
    pub mcp_auth_probe_in_flight: bool,
    pub mcp_auth_tone: Tone,
    pub update: UpdatePayload<'a>,
    pub pending_device_action: Option<DeviceAction>,
    pub app_name: &'static str,
    pub sign_in_label: &'static str,
    pub sign_in_hint: &'static str,
    pub docs_url: &'static str,
    pub contact_email: &'static str,
    pub pitch_head: &'static str,
    pub pitch_body: &'static str,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts-export", ts(flatten))]
    pub hosts: hosts::HostsPayload<'a>,
}
