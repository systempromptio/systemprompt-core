//! W3C trace context carried onto upstream provider requests.
//!
//! The dispatch scopes the request's [`TraceId`] around the provider send, and
//! `send_checked` stamps a `traceparent` (same trace id, a new parent
//! span id per upstream send), so a provider's own request logs join the
//! caller's trace. A trace id with no W3C form is not propagated. The sampled
//! flag is the caller's when it sent one in `traceparent`; a trace the gateway
//! originated is marked sampled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;

use systemprompt_identifiers::{TraceId, headers};

tokio::task_local! {
    static TRACE: Option<(TraceId, bool)>;
}

pub async fn with_trace<F: Future>(
    trace_id: Option<TraceId>,
    sampled: Option<bool>,
    fut: F,
) -> F::Output {
    let scoped = trace_id.map(|id| (id, sampled.unwrap_or(true)));
    TRACE.scope(scoped, fut).await
}

#[must_use]
pub fn current_traceparent() -> Option<String> {
    TRACE
        .try_with(|trace| {
            trace
                .as_ref()
                .and_then(|(id, sampled)| id.traceparent_sampled(*sampled))
        })
        .ok()
        .flatten()
}

pub(crate) fn stamp(req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    match current_traceparent() {
        Some(value) => req.header(headers::TRACEPARENT, value),
        None => req,
    }
}
