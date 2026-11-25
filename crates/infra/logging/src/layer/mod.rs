//! `tracing` subscriber layer that persists events to the database.
//!
//! [`DatabaseLayer`] buffers log events off the hot path and batch-inserts them
//! from a background task, flushing on a size threshold, a timer, or
//! immediately on an error. The task belongs to the [`LogWriterHandle`]
//! returned alongside the layer; awaiting [`LogWriterHandle::shutdown`] flushes
//! whatever is still buffered or queued and stops the task.
//! [`ProxyDatabaseLayer`] is the proxy-side variant. Both share a
//! `LayerShared`: the instance label stamped on every entry and the count of
//! entries dropped on a full or closed queue.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "otel")]
pub(crate) mod export;
#[cfg(not(feature = "otel"))]
pub(crate) mod export {
    use systemprompt_models::audit_spans::{
        GovernanceSpanRecord, RequestSpanRecord, ToolCallSpanRecord,
    };
    use tracing_subscriber::layer::Context;

    use crate::models::LogEntry;

    #[derive(Debug, Clone, Copy, Default)]
    pub(crate) struct ExportSlot;

    #[expect(
        clippy::unused_self,
        reason = "without the otel feature there is no feed; the slot keeps the exporting shape"
    )]
    impl ExportSlot {
        pub(crate) const fn log(self, _entry: &LogEntry) {}

        pub(crate) const fn open<S>(self, _id: &tracing::span::Id, _ctx: &Context<'_, S>) {}

        pub(crate) const fn close<S>(self, _id: &tracing::span::Id, _ctx: &Context<'_, S>) {}

        pub(crate) const fn request(self, _span: &RequestSpanRecord) {}

        pub(crate) const fn tool_call(self, _span: &ToolCallSpanRecord) {}

        pub(crate) const fn governance(self, _span: &GovernanceSpanRecord) {}
    }
}
#[cfg(feature = "otel")]
mod otlp_span;
mod proxy;
mod visitor;
mod writer;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::mpsc;
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

pub use proxy::ProxyDatabaseLayer;
use proxy::{build_log_entry, record_span_fields, update_span_fields};
pub use writer::{LogWriterHandle, LogWriterShutdownError};

use self::writer::LogCommand;
use crate::models::{LogEntry, LogLevel};
use crate::repository::LoggingRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::InstanceId;
use systemprompt_models::LateBound;

const CHANNEL_CAPACITY: usize = 8192;

#[derive(Debug, Clone, Default)]
pub(crate) struct InstanceLabel(LateBound<InstanceId>);

impl InstanceLabel {
    pub(crate) fn get(&self) -> Option<&InstanceId> {
        self.0.get().map(|id| &**id)
    }

    pub(crate) fn set(&self, instance_id: InstanceId) {
        let label = instance_id.clone();
        if self.0.bind(Arc::new(instance_id)).is_err() {
            tracing::debug!(instance_id = %label, "logging instance label already set; ignoring");
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LayerShared {
    pub(crate) instance: InstanceLabel,
    dropped: Arc<AtomicU64>,
    pub(crate) export: export::ExportSlot,
}

impl LayerShared {
    pub(crate) fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub(crate) fn record_drop(&self) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }
}

pub struct DatabaseLayer {
    sender: mpsc::Sender<LogCommand>,
    shared: LayerShared,
}

impl std::fmt::Debug for DatabaseLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseLayer")
            .field("dropped", &self.dropped_entries())
            .finish_non_exhaustive()
    }
}

impl DatabaseLayer {
    pub fn new(db_pool: &DbPool) -> (Self, LogWriterHandle) {
        Self::with_shared(db_pool, LayerShared::default())
    }

    pub(crate) fn with_shared(db_pool: &DbPool, shared: LayerShared) -> (Self, LogWriterHandle) {
        let (sender, receiver) = mpsc::channel(CHANNEL_CAPACITY);
        let writer =
            LogWriterHandle::spawn(LoggingRepository::new(db_pool), sender.clone(), receiver);
        (Self { sender, shared }, writer)
    }

    #[must_use]
    pub fn dropped_entries(&self) -> u64 {
        self.shared.dropped()
    }

    pub fn enqueue(&self, mut entry: LogEntry) {
        if entry.instance_id.is_none() {
            entry.instance_id = self.shared.instance.get().cloned();
        }
        self.shared.export.log(&entry);
        let is_error = entry.level == LogLevel::Error;
        if self
            .sender
            .try_send(LogCommand::Entry(Box::new(entry)))
            .is_err()
        {
            self.shared.record_drop();
            return;
        }
        if is_error {
            // Why: a full queue already forces the flush this would request, and
            // tracing here would re-enter this layer.
            let _flush_requested = self.sender.try_send(LogCommand::FlushNow).is_ok();
        }
    }
}

impl<S> Layer<S> for DatabaseLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: Context<'_, S>,
    ) {
        record_span_fields(attrs, id, &ctx);
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: Context<'_, S>,
    ) {
        update_span_fields(id, values, &ctx);
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if let Some(entry) = build_log_entry(event, &ctx) {
            self.enqueue(entry);
        }
    }
}
