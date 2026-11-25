//! The live OTLP export slot the layers share: empty until
//! [`crate::LogSink::start_otlp_export`] binds a feed, and then offered every
//! persisted entry, every span open and close, and every audit span.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, OnceLock};

use systemprompt_models::audit_spans::{
    GovernanceSpanRecord, RequestSpanRecord, ToolCallSpanRecord,
};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use crate::models::LogEntry;
use crate::otlp::OtlpFeed;

#[derive(Debug, Clone, Default)]
pub(crate) struct ExportSlot(Arc<OnceLock<OtlpFeed>>);

impl ExportSlot {
    pub(crate) fn bind(&self, feed: impl FnOnce() -> OtlpFeed) -> bool {
        if self.0.get().is_some() {
            return false;
        }
        self.0.set(feed()).is_ok()
    }

    pub(crate) fn dropped(&self) -> Option<u64> {
        self.0.get().map(OtlpFeed::dropped)
    }

    pub(crate) fn log(&self, entry: &LogEntry) {
        if let Some(feed) = self.0.get() {
            feed.log(entry);
        }
    }

    pub(crate) fn open<S>(&self, id: &tracing::span::Id, ctx: &Context<'_, S>)
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        if self.0.get().is_some() {
            super::otlp_span::open(id, ctx);
        }
    }

    pub(crate) fn close<S>(&self, id: &tracing::span::Id, ctx: &Context<'_, S>)
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        if let Some(feed) = self.0.get() {
            super::otlp_span::close(feed, id, ctx);
        }
    }

    pub(crate) fn request(&self, span: &RequestSpanRecord) {
        if let Some(feed) = self.0.get() {
            feed.audit_span(crate::otlp::request_span(span));
        }
    }

    pub(crate) fn tool_call(&self, span: &ToolCallSpanRecord) {
        if let Some(feed) = self.0.get() {
            feed.audit_span(crate::otlp::tool_call_span(span));
        }
    }

    pub(crate) fn governance(&self, span: &GovernanceSpanRecord) {
        if let Some(feed) = self.0.get() {
            feed.audit_span(crate::otlp::governance_span(span));
        }
    }
}
