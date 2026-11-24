//! Live OTLP export of this instance's own telemetry.
//!
//! When the profile names a collector (`observability.otlp`), the
//! installed `tracing` pipeline feeds an [`OtlpFeed`]: every log entry the
//! database layer persists becomes an OTLP log record, and every span that
//! carries a platform `trace_id` becomes an OTLP span when it closes. The
//! audit producers add their own typed spans through
//! [`systemprompt_models::audit_spans::AuditSpanSink`] (`audit_spans`): the
//! `ai_request` span of every settled gateway request and its `tool_call` and
//! `governance_decision` children. The
//! feed is a bounded queue; a full queue drops the item and counts it in
//! [`DROPPED_TOTAL`] rather than slowing the code that emitted it. One owned
//! task drains the queue and posts a batch every `batch_seconds` or every
//! [`BATCH_ITEMS`] items, whichever comes first, and flushes what is left
//! when the process shuts down. Delivery is at-most-once: a batch the
//! collector refuses after its retries is dropped and counted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod attrs;
mod audit_spans;
mod exporter;
mod ids;
mod records;
mod transport;

pub use audit_spans::{
    GOVERNANCE_SPAN, REQUEST_SPAN, TOOL_SPAN, governance_span, request_span, tool_call_span,
};
pub use exporter::{BATCH_ITEMS, DROPPED_TOTAL, OtlpFeed, QUEUE_CAPACITY};
pub use ids::{span_id_bytes, trace_id_bytes, unix_nanos};
pub(crate) use records::ClosedSpan;
pub use records::{SPAN_KIND, severity_number, to_log_record};
pub use transport::{BATCHES_TOTAL, RETRY_DELAYS, is_retryable};
