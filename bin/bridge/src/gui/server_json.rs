//! JSON payload shapes served to the GUI webview (state and proxy stats).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::Ordering;

use serde::Serialize;
use serde_json::{Value, json};

use crate::gui::state::{
    AppStateSnapshot, CachedToken, GatewayCode, GatewayStatus, HealthCode, IdentityCode,
    OverallCode, TokenCode, VerifiedIdentity,
};
use crate::proxy::mcp_probe::McpServerAuth;
use crate::validate::{CheckLine, ValidationCode, ValidationReport};
use crate::verdict::{Tone, Verdict};

pub fn snapshot_value(snap: &AppStateSnapshot) -> Value {
    serde_json::to_value(StatePayload::from(snap)).unwrap_or(Value::Null)
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

// Why: The one place the auth verdict crosses to the UI.
//
// Why computed here rather than re-derived in JavaScript: the front end used
// to test the state name itself, against a variant that does not exist, and
// so declared every healthy server broken. Shipping the verdict beside the
// state leaves the UI nothing to get wrong.
#[derive(Serialize)]
struct McpServerAuthPayload<'a> {
    #[serde(flatten)]
    server: &'a McpServerAuth,
    verdict: Verdict<crate::proxy::mcp_probe::McpAuthState>,
    needs_sign_in: bool,
    conclusive: bool,
    shows_tools: bool,
}

fn mcp_servers_payload(snap: &AppStateSnapshot) -> Vec<McpServerAuthPayload<'_>> {
    snap.mcp_auth
        .iter()
        .map(|server| McpServerAuthPayload {
            server,
            verdict: server.state.verdict(),
            needs_sign_in: server.state.needs_sign_in(),
            conclusive: server.state.is_conclusive(),
            shows_tools: server.state.shows_tools(),
        })
        .collect()
}

#[derive(Serialize)]
struct CheckLinePayload<'a> {
    tone: Tone,
    label: &'a str,
    value: &'a str,
}

#[derive(Serialize)]
struct ValidationPayload<'a> {
    lines: Vec<CheckLinePayload<'a>>,
    any_failed: bool,
    verdict: Verdict<ValidationCode>,
}

impl<'a> From<&'a ValidationReport> for ValidationPayload<'a> {
    fn from(r: &'a ValidationReport) -> Self {
        Self {
            lines: r
                .lines
                .iter()
                .map(
                    |CheckLine {
                         level,
                         label,
                         value,
                     }| CheckLinePayload {
                        tone: level.tone(),
                        label,
                        value,
                    },
                )
                .collect(),
            any_failed: r.any_failed,
            verdict: r.verdict(),
        }
    }
}

#[derive(Serialize)]
struct UpdatePayload<'a> {
    #[serde(flatten)]
    state: &'a crate::update::UpdateUiState,
    tone: Tone,
    can_install: bool,
    can_restart: bool,
    in_progress: bool,
}

impl<'a> From<&'a crate::update::UpdateUiState> for UpdatePayload<'a> {
    fn from(state: &'a crate::update::UpdateUiState) -> Self {
        Self {
            state,
            tone: state.tone(),
            can_install: state.can_install(),
            can_restart: state.can_restart(),
            in_progress: state.in_progress(),
        }
    }
}

pub fn proxy_stats_value() -> Value {
    serde_json::to_value(ProxyStatsPayload::current()).unwrap_or(Value::Null)
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
    provider_health: &'a [crate::auth::types::ProviderHealth],
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

impl<'a> From<&'a AppStateSnapshot> for StatePayload<'a> {
    fn from(snap: &'a AppStateSnapshot) -> Self {
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
            proxy_stats: ProxyStatsPayload::current(),
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

#[derive(Serialize, Default)]
pub(crate) struct ProxyStatsPayload {
    forwarded_total: u64,
    messages_total: u64,
    tokens_in_total: u64,
    tokens_out_total: u64,
    last_status: u64,
    last_latency_ms: u64,
    last_forwarded_at_unix: u64,
}

impl ProxyStatsPayload {
    fn current() -> Self {
        let Some(handle) = crate::proxy::handle() else {
            return Self::default();
        };
        let s = &handle.stats;
        Self {
            forwarded_total: s.forwarded_total.load(Ordering::Relaxed),
            messages_total: s.messages_total.load(Ordering::Relaxed),
            tokens_in_total: s.tokens_in_total.load(Ordering::Relaxed),
            tokens_out_total: s.tokens_out_total.load(Ordering::Relaxed),
            last_status: s.last_status.load(Ordering::Relaxed),
            last_latency_ms: s.last_latency_ms.load(Ordering::Relaxed),
            last_forwarded_at_unix: s.last_forwarded_at_unix.load(Ordering::Relaxed),
        }
    }
}

#[derive(Serialize)]
struct CachedTokenPayload {
    ttl_seconds: u64,
    length: usize,
}

impl From<&CachedToken> for CachedTokenPayload {
    fn from(t: &CachedToken) -> Self {
        Self {
            ttl_seconds: t.ttl_seconds,
            length: t.length,
        }
    }
}

#[derive(Serialize)]
struct GatewayStatusPayload<'a> {
    #[serde(flatten)]
    verdict: Verdict<GatewayCode>,
    settled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
}

impl<'a> From<&'a GatewayStatus> for GatewayStatusPayload<'a> {
    fn from(s: &'a GatewayStatus) -> Self {
        let (latency_ms, reason) = match s {
            GatewayStatus::Reachable { latency_ms } => (Some(*latency_ms), None),
            GatewayStatus::Unreachable { reason } => (None, Some(reason.as_str())),
            GatewayStatus::Unknown | GatewayStatus::Probing => (None, None),
        };
        Self {
            verdict: s.verdict(),
            settled: s.settled(),
            latency_ms,
            reason,
        }
    }
}

#[derive(Serialize)]
struct VerifiedIdentityPayload<'a> {
    email: Option<&'a str>,
    user_id: Option<&'a str>,
    tenant_id: Option<&'a str>,
    exp_unix: Option<u64>,
    verified_at_unix: u64,
}

impl<'a> From<&'a VerifiedIdentity> for VerifiedIdentityPayload<'a> {
    fn from(v: &'a VerifiedIdentity) -> Self {
        Self {
            email: v.email.as_deref(),
            user_id: v
                .user_id
                .as_ref()
                .map(systemprompt_identifiers::UserId::as_str),
            tenant_id: v
                .tenant_id
                .as_ref()
                .map(systemprompt_identifiers::TenantId::as_str),
            exp_unix: v.exp_unix,
            verified_at_unix: v.verified_at_unix,
        }
    }
}
