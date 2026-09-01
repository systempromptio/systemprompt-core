//! JSON payload shapes served to the GUI webview (state and proxy stats).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use serde_json::{Value, json};

mod payloads;

use payloads::{
    CachedTokenPayload, GatewayStatusPayload, McpServerAuthPayload, ProxyStatsPayload,
    UpdatePayload, ValidationPayload, VerifiedIdentityPayload, mcp_servers_payload,
};

use crate::gui::state::{AppStateSnapshot, HealthCode, IdentityCode, OverallCode, TokenCode};
use crate::verdict::{Tone, Verdict};

pub fn snapshot_value(snap: &AppStateSnapshot, proxy: &crate::proxy::ProxyHandle) -> Value {
    serde_json::to_value(StatePayload::build(snap, proxy)).unwrap_or(Value::Null)
}

pub fn identity_value(snap: &AppStateSnapshot) -> Value {
    snap.verified_identity.as_ref().map_or(Value::Null, |v| {
        serde_json::to_value(VerifiedIdentityPayload::from(v)).unwrap_or(Value::Null)
    })
}

pub fn single_host_value(snap: &AppStateSnapshot, host_id: &str) -> Value {
    let payload = crate::gui::hosts::serde::single_host_payload(snap, host_id);
    serde_json::to_value(payload).unwrap_or(Value::Null)
}

pub fn local_proxy_value(snap: &AppStateSnapshot) -> Value {
    serde_json::to_value(crate::gui::hosts::serde::ProxyPayload::from(
        &snap.hosts.local_proxy,
    ))
    .unwrap_or(Value::Null)
}

pub fn mcp_auth_value(snap: &AppStateSnapshot) -> Value {
    json!({
        "servers": mcp_servers_payload(snap),
        "probing": snap.mcp_auth_probe_in_flight,
        "tone": snap.mcp_auth_tone(),
    })
}

pub fn proxy_stats_value(proxy: &crate::proxy::ProxyHandle) -> Value {
    serde_json::to_value(ProxyStatsPayload::current(proxy)).unwrap_or(Value::Null)
}

#[derive(Serialize)]
struct StatePayload<'a> {
    gateway_url: &'a str,
    config_file: &'a str,
    pat_file: &'a str,
    config_present: bool,
    pat_present: bool,
    plugins_dir: Option<&'a str>,
    last_sync_summary: Option<&'a str>,
    last_sync_report: Option<&'a crate::sync::SyncSummary>,
    skill_count: Option<usize>,
    agent_count: Option<usize>,
    plugin_count: Option<usize>,
    malformed_plugin_count: Option<usize>,
    last_validation: Option<ValidationPayload<'a>>,
    last_validation_at_unix: Option<u64>,
    health: Verdict<HealthCode>,
    provider_health: &'a [crate::gateway::types::ProviderHealth],
    sync_in_flight: bool,
    cached_token: Option<CachedTokenPayload>,
    token: Verdict<TokenCode>,
    gateway_status: GatewayStatusPayload<'a>,
    verified_identity: Option<VerifiedIdentityPayload<'a>>,
    identity: Verdict<IdentityCode>,
    cloud_tone: Tone,
    overall: Verdict<OverallCode>,
    signed_in: bool,
    last_probe_at_unix: Option<u64>,
    proxy_stats: ProxyStatsPayload,
    mcp_auth: Vec<McpServerAuthPayload<'a>>,
    mcp_auth_probe_in_flight: bool,
    mcp_auth_tone: Tone,
    update: UpdatePayload<'a>,

    sign_in_label: &'static str,
    sign_in_hint: &'static str,

    #[serde(flatten)]
    hosts: crate::gui::hosts::serde::HostsPayload<'a>,
}

impl<'a> StatePayload<'a> {
    fn build(snap: &'a AppStateSnapshot, proxy: &crate::proxy::ProxyHandle) -> Self {
        Self {
            gateway_url: snap.gateway_url.as_str(),
            config_file: snap.config_file.as_str(),
            pat_file: snap.pat_file.as_str(),
            config_present: snap.config_present,
            pat_present: snap.pat_present,
            plugins_dir: snap.plugins_dir.as_deref(),
            last_sync_summary: snap.last_sync_summary.as_deref(),
            last_sync_report: snap.last_sync_report.as_ref(),
            skill_count: snap.skill_count,
            agent_count: snap.agent_count,
            plugin_count: snap.plugin_count,
            malformed_plugin_count: snap.malformed_plugin_count,
            last_validation: snap.last_validation.as_ref().map(ValidationPayload::from),
            last_validation_at_unix: snap.last_validation_at_unix,
            health: snap.health_verdict(),
            provider_health: &snap.provider_health,
            sync_in_flight: snap.sync_in_flight,
            cached_token: snap.cached_token.as_ref().map(CachedTokenPayload::from),
            token: snap.token_verdict(),
            gateway_status: GatewayStatusPayload::from(&snap.gateway_status),
            verified_identity: snap
                .verified_identity
                .as_ref()
                .map(VerifiedIdentityPayload::from),
            identity: snap.identity_verdict(),
            cloud_tone: snap.cloud_tone(),
            overall: snap.overall_verdict(),
            signed_in: snap.signed_in(),
            last_probe_at_unix: snap.last_probe_at_unix,
            proxy_stats: ProxyStatsPayload::current(proxy),
            mcp_auth: mcp_servers_payload(snap),
            mcp_auth_probe_in_flight: snap.mcp_auth_probe_in_flight,
            mcp_auth_tone: snap.mcp_auth_tone(),
            update: UpdatePayload::from(&snap.update),

            sign_in_label: crate::brand::brand().sign_in_label,
            sign_in_hint: crate::brand::brand().sign_in_hint,

            hosts: crate::gui::hosts::serde::payload(snap),
        }
    }
}
