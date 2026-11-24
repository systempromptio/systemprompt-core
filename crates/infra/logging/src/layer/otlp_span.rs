//! Span timing for the live OTLP feed: a span is stamped with an OTLP span id
//! and its start time when it opens, and handed to the feed when it closes,
//! with the attribution it inherits from its ancestors. A span under no
//! platform trace is not exported.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::TraceId;
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use super::visitor::extract_span_context;
use crate::otlp::{ClosedSpan, OtlpFeed, span_id_bytes};

struct SpanStart {
    span_id: Vec<u8>,
    start: DateTime<Utc>,
}

pub(super) fn open<S>(id: &tracing::span::Id, ctx: &Context<'_, S>)
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    if let Some(span) = ctx.span(id) {
        span.extensions_mut().insert(SpanStart {
            span_id: span_id_bytes("span", &uuid::Uuid::new_v4().to_string()),
            start: Utc::now(),
        });
    }
}

pub(super) fn close<S>(feed: &OtlpFeed, id: &tracing::span::Id, ctx: &Context<'_, S>)
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let Some(span) = ctx.span(id) else {
        return;
    };
    let Some((span_id, start)) = span
        .extensions()
        .get::<SpanStart>()
        .map(|stamp| (stamp.span_id.clone(), stamp.start))
    else {
        return;
    };
    let parent_span_id = span
        .parent()
        .and_then(|parent| {
            parent
                .extensions()
                .get::<SpanStart>()
                .map(|stamp| stamp.span_id.clone())
        })
        .unwrap_or_default();
    let name = span.metadata().name();
    let target = span.metadata().target();
    let context = extract_span_context(span);
    let Some(trace) = context.trace.filter(|t| !t.is_empty()) else {
        return;
    };
    feed.span(ClosedSpan {
        name,
        target,
        trace: TraceId::new(trace),
        span_id,
        parent_span_id,
        start,
        end: Utc::now(),
        user: context.user,
        session: context.session,
        task: context.task,
        context: context.context,
        client: context.client,
    });
}
