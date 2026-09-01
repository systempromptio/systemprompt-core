//! The sub-payloads `StatePayload` is assembled from: each is a wire view of
//! one internal type with its verdict beside it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::Ordering;

use serde::Serialize;

use crate::gui::state::{
    AppStateSnapshot, CachedToken, GatewayCode, GatewayStatus, VerifiedIdentity,
};
use crate::proxy::mcp_probe::McpServerAuth;
use crate::validate::{CheckLine, ValidationCode, ValidationReport};
use crate::verdict::{Tone, Verdict};

// Why: The one place the auth verdict crosses to the UI.
//
// Why computed here rather than re-derived in JavaScript: the front end used
// to test the state name itself, against a variant that does not exist, and
// so declared every healthy server broken. Shipping the verdict beside the
// state leaves the UI nothing to get wrong.
#[derive(Serialize)]
pub(super) struct McpServerAuthPayload<'a> {
    #[serde(flatten)]
    server: &'a McpServerAuth,
    verdict: Verdict<crate::proxy::mcp_probe::McpAuthState>,
    needs_sign_in: bool,
    conclusive: bool,
    shows_tools: bool,
}

pub(super) fn mcp_servers_payload(snap: &AppStateSnapshot) -> Vec<McpServerAuthPayload<'_>> {
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
pub(super) struct CheckLinePayload<'a> {
    tone: Tone,
    label: &'a str,
    value: &'a str,
}

#[derive(Serialize)]
pub(super) struct ValidationPayload<'a> {
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
pub(super) struct UpdatePayload<'a> {
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
    pub(super) fn current() -> Self {
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
pub(super) struct CachedTokenPayload {
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
pub(super) struct GatewayStatusPayload<'a> {
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
pub(super) struct VerifiedIdentityPayload<'a> {
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
