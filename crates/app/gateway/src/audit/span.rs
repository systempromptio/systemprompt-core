//! The request's `ai_request` audit span, handed to the logging sink the
//! moment its terminal receipt settles. The sink offers it to the live OTLP
//! export when one runs; handing it over never blocks or fails the request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{Duration, Utc};
use systemprompt_models::audit_spans::{AuditSpanSink, RequestSpanRecord};
use systemprompt_traits::RequestStatus;

use super::GatewayAudit;
use crate::protocol::canonical::CanonicalUsage;

pub(super) struct SettledOutcome<'a> {
    pub status: RequestStatus,
    pub error: Option<&'a str>,
    pub usage: Option<[u32; 6]>,
    pub cost_microdollars: i64,
    pub latency_ms: i32,
    pub upstream_latency_ms: Option<i32>,
    pub finish_reason: Option<&'a str>,
}

fn tokens(usage: [u32; 6], index: usize) -> i32 {
    i32::try_from(usage[index]).unwrap_or(i32::MAX)
}

pub(super) const fn usage_columns(usage: &CanonicalUsage, billable: u32) -> [u32; 6] {
    [
        usage.input_tokens,
        usage.output_tokens,
        usage.cache_read_tokens,
        usage.cache_creation_tokens,
        usage.reasoning_tokens,
        billable,
    ]
}

impl GatewayAudit {
    pub(super) fn emit_request_span(&self, outcome: &SettledOutcome<'_>) {
        let record = self.build_record();
        let completed_at = Utc::now();
        let route_match = self.route_match.lock().ok().and_then(|slot| slot.clone());
        let span = RequestSpanRecord {
            request_id: self.ctx.ai_request_id.clone(),
            request_kind: record.request_kind.as_str().to_owned(),
            status: outcome.status,
            error_message: outcome.error.map(str::to_owned),
            user_id: self.ctx.user_id.clone(),
            actor: record.actor.clone(),
            session_id: self.ctx.session_id.clone(),
            context_id: self.ctx.context_id.clone(),
            trace_id: self.ctx.trace_id.clone(),
            provider: Some(self.ctx.provider.clone()),
            served_provider: Some(self.served_provider()),
            requested_model: self.ctx.requested_model.clone(),
            model: Some(self.effective_model()),
            route_match,
            input_tokens: outcome.usage.map(|u| tokens(u, 0)),
            output_tokens: outcome.usage.map(|u| tokens(u, 1)),
            cache_read_tokens: outcome.usage.map(|u| tokens(u, 2)),
            cache_creation_tokens: outcome.usage.map(|u| tokens(u, 3)),
            cost_microdollars: outcome.cost_microdollars,
            latency_ms: Some(outcome.latency_ms),
            upstream_latency_ms: outcome.upstream_latency_ms,
            finish_reason: outcome.finish_reason.map(str::to_owned),
            client_kind: self.ctx.origin.client.as_str().to_owned(),
            wire_protocol: self.ctx.origin.wire.as_str().to_owned(),
            instance_id: self.logging.instance_id().cloned(),
            api_key_id: self.ctx.attribution.api_key_id.clone(),
            attributions: self.ctx.attribution.entries.clone(),
            started_at: completed_at - Duration::milliseconds(i64::from(outcome.latency_ms)),
            completed_at,
        };
        self.logging.sink().request(span);
    }
}
