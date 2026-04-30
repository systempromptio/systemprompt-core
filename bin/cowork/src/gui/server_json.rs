use std::sync::atomic::Ordering;

use serde::Serialize;

use crate::gui::state::{AppStateSnapshot, CachedToken, GatewayStatus, VerifiedIdentity};

pub(crate) fn snapshot_to_json(snap: &AppStateSnapshot) -> Result<String, serde_json::Error> {
    serde_json::to_string(&StatePayload::from(snap))
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
    skill_count: Option<usize>,
    agent_count: Option<usize>,
    plugin_count: Option<usize>,
    sync_in_flight: bool,
    last_action_message: Option<&'a str>,
    cached_token: Option<CachedTokenPayload>,
    gateway_status: GatewayStatusPayload<'a>,
    verified_identity: Option<VerifiedIdentityPayload<'a>>,
    signed_in: bool,
    last_probe_at_unix: Option<u64>,
    proxy_stats: ProxyStatsPayload,
    
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
            skill_count: snap.skill_count,
            agent_count: snap.agent_count,
            plugin_count: snap.plugin_count,
            sync_in_flight: snap.sync_in_flight,
            last_action_message: snap.last_action_message.as_deref(),
            cached_token: snap.cached_token.as_ref().map(CachedTokenPayload::from),
            gateway_status: GatewayStatusPayload::from(&snap.gateway_status),
            verified_identity: snap
                .verified_identity
                .as_ref()
                .map(VerifiedIdentityPayload::from),
            signed_in: snap.signed_in(),
            last_probe_at_unix: snap.last_probe_at_unix,
            proxy_stats: ProxyStatsPayload::current(),
            
            hosts: crate::gui::hosts::serde::payload(snap),
        }
    }
}

#[derive(Serialize, Default)]
struct ProxyStatsPayload {
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
#[serde(tag = "state", rename_all = "lowercase")]
enum GatewayStatusPayload<'a> {
    Unknown,
    Probing,
    Reachable { latency_ms: u64 },
    Unreachable { reason: &'a str },
}

impl<'a> From<&'a GatewayStatus> for GatewayStatusPayload<'a> {
    fn from(s: &'a GatewayStatus) -> Self {
        match s {
            GatewayStatus::Unknown => Self::Unknown,
            GatewayStatus::Probing => Self::Probing,
            GatewayStatus::Reachable { latency_ms } => Self::Reachable {
                latency_ms: *latency_ms,
            },
            GatewayStatus::Unreachable { reason } => Self::Unreachable { reason },
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
            user_id: v.user_id.as_deref(),
            tenant_id: v.tenant_id.as_deref(),
            exp_unix: v.exp_unix,
            verified_at_unix: v.verified_at_unix,
        }
    }
}
