//! The sub-payloads `StatePayload` is assembled from: each is a wire view of
//! one internal type with its verdict beside it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::Ordering;

use serde::Serialize;

use crate::proxy::mcp_probe::{McpAuthState, McpServerAuth};
use crate::validate::{CheckLine, ValidationCode, ValidationReport};
use crate::verdict::{Tone, Verdict};
use crate::wire::codes::GatewayCode;

// Why: The one place the auth verdict crosses to the UI.
//
// Why computed here rather than re-derived in JavaScript: the front end used
// to test the state name itself, against a variant that does not exist, and
// so declared every healthy server broken. Shipping the verdict beside the
// state leaves the UI nothing to get wrong.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct McpServerAuthPayload<'a> {
    #[serde(flatten)]
    #[cfg_attr(feature = "ts-export", ts(flatten))]
    pub server: &'a McpServerAuth,
    pub verdict: Verdict<McpAuthState>,
    pub needs_sign_in: bool,
    pub conclusive: bool,
    pub shows_tools: bool,
}

impl<'a> From<&'a McpServerAuth> for McpServerAuthPayload<'a> {
    fn from(server: &'a McpServerAuth) -> Self {
        Self {
            server,
            verdict: server.state.verdict(),
            needs_sign_in: server.state.needs_sign_in(),
            conclusive: server.state.is_conclusive(),
            shows_tools: server.state.shows_tools(),
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct CheckLinePayload<'a> {
    pub tone: Tone,
    pub label: &'a str,
    pub value: &'a str,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct ValidationPayload<'a> {
    pub lines: Vec<CheckLinePayload<'a>>,
    pub any_failed: bool,
    pub verdict: Verdict<ValidationCode>,
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

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct UpdatePayload<'a> {
    #[serde(flatten)]
    #[cfg_attr(feature = "ts-export", ts(flatten))]
    pub state: &'a crate::update::UpdateUiState,
    pub tone: Tone,
    pub can_install: bool,
    pub can_restart: bool,
    pub in_progress: bool,
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

#[derive(Debug, Clone, Copy, Serialize, Default)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct ProxyStatsPayload {
    pub forwarded_total: u64,
    pub messages_total: u64,
    pub tokens_in_total: u64,
    pub tokens_out_total: u64,
    pub last_status: u64,
    pub last_latency_ms: u64,
    pub last_forwarded_at_unix: u64,
}

impl ProxyStatsPayload {
    #[must_use]
    pub fn current(proxy: &crate::proxy::ProxyHandle) -> Self {
        let Some(served) = proxy.served() else {
            return Self::default();
        };
        let s = &served.stats;
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

#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct CachedTokenPayload {
    pub ttl_seconds: u64,
    pub length: usize,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct GatewayStatusPayload<'a> {
    #[serde(flatten)]
    #[cfg_attr(feature = "ts-export", ts(flatten))]
    pub verdict: Verdict<GatewayCode>,
    pub settled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub reason: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct VerifiedIdentityPayload<'a> {
    pub email: Option<&'a str>,
    pub user_id: Option<&'a str>,
    pub tenant_id: Option<&'a str>,
    pub exp_unix: Option<u64>,
    pub verified_at_unix: u64,
}
