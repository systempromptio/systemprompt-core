//! The GUI wire: every payload the webview is written against, exported to
//! TypeScript by `just bridge-bindings` and checked by `bridge-bindings-check`.
//!
//! Nothing here touches winit, wry or the tray, so the module builds on every
//! target — the front end used to be written against JSON that only a
//! Windows or macOS build could name.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod codes;
pub mod first_run;
pub mod hosts;
pub mod ipc;
pub mod payloads;

use serde::Serialize;

use crate::verdict::{Tone, Verdict};
use codes::{HealthCode, IdentityCode, OverallCode, TokenCode};
use payloads::{
    CachedTokenPayload, GatewayStatusPayload, McpServerAuthPayload, ProxyStatsPayload,
    UpdatePayload, ValidationPayload, VerifiedIdentityPayload,
};

/// The whole state snapshot as the webview receives it on `state.snapshot`
/// and every `state.changed`.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct StatePayload<'a> {
    pub gateway_url: &'a str,
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
    pub app_name: &'static str,
    pub sign_in_label: &'static str,
    pub sign_in_hint: &'static str,
    pub docs_url: &'static str,
    pub contact_email: &'static str,
    #[serde(flatten)]
    #[cfg_attr(feature = "ts-export", ts(flatten))]
    pub hosts: hosts::HostsPayload<'a>,
}
