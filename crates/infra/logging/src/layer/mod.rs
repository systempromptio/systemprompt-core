//! `tracing` subscriber layer that persists events to the database.
//!
//! [`DatabaseLayer`] buffers log events off the hot path and batch-inserts them
//! from a background task, flushing on a size threshold, a timer, or
//! immediately on an error. The task belongs to the [`LogWriterHandle`]
//! returned alongside the layer; awaiting [`LogWriterHandle::shutdown`] flushes
//! whatever is still buffered or queued and stops the task.
//! [`ProxyDatabaseLayer`] is the proxy-side variant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod proxy;
mod visitor;
mod writer;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

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

const CHANNEL_CAPACITY: usize = 8192;

static BACKGROUND_SENDER: OnceLock<mpsc::Sender<LogCommand>> = OnceLock::new();
static BACKGROUND_DROPPED: AtomicU64 = AtomicU64::new(0);

pub fn enqueue_background(entry: LogEntry) {
    let Some(sender) = BACKGROUND_SENDER.get() else {
        BACKGROUND_DROPPED.fetch_add(1, Ordering::Relaxed);
        return;
    };
    let is_error = entry.level == LogLevel::Error;
    if sender.try_send(LogCommand::Entry(Box::new(entry))).is_err() {
        BACKGROUND_DROPPED.fetch_add(1, Ordering::Relaxed);
        return;
    }
    if is_error {
        // Why: a full queue already forces the flush this would request, and
        // tracing here would re-enter this layer.
        let _flush_requested = sender.try_send(LogCommand::FlushNow).is_ok();
    }
}

struct LogChannel {
    sender: mpsc::Sender<LogCommand>,
    dropped: Arc<AtomicU64>,
}

impl LogChannel {
    fn new(capacity: usize) -> (Self, mpsc::Receiver<LogCommand>) {
        let (sender, receiver) = mpsc::channel(capacity);
        let channel = Self {
            sender,
            dropped: Arc::new(AtomicU64::new(0)),
        };
        (channel, receiver)
    }

    fn send(&self, command: LogCommand) {
        if let Err(mpsc::error::TrySendError::Full(_)) = self.sender.try_send(command) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

pub struct DatabaseLayer {
    channel: LogChannel,
}

impl std::fmt::Debug for DatabaseLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseLayer")
            .field("dropped", &self.channel.dropped())
            .finish_non_exhaustive()
    }
}

impl DatabaseLayer {
    pub fn new(db_pool: &DbPool) -> (Self, LogWriterHandle) {
        let (channel, receiver) = LogChannel::new(CHANNEL_CAPACITY);

        BACKGROUND_SENDER.get_or_init(|| channel.sender.clone());

        let writer = LogWriterHandle::spawn(
            LoggingRepository::new(db_pool),
            channel.sender.clone(),
            receiver,
        );

        (Self { channel }, writer)
    }
}

impl DatabaseLayer {
    fn send_entry(&self, entry: LogEntry) {
        let is_error = entry.level == LogLevel::Error;
        self.channel.send(LogCommand::Entry(Box::new(entry)));
        if is_error {
            self.channel.send(LogCommand::FlushNow);
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
            self.send_entry(entry);
        }
    }
}
