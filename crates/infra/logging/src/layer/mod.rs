//! `tracing` subscriber layer that persists events to the database.
//!
//! [`DatabaseLayer`] buffers log events off the hot path and batch-inserts them
//! from a background task, flushing on a size threshold, a timer, or
//! immediately on an error. [`ProxyDatabaseLayer`] is the proxy-side variant.

mod proxy;
mod visitor;

use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::sync::mpsc;
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

pub use proxy::ProxyDatabaseLayer;
use proxy::{build_log_entry, record_span_fields, update_span_fields};

use crate::models::{LogEntry, LogLevel};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ClientId, ContextId, TaskId};

const BUFFER_FLUSH_SIZE: usize = 100;
const BUFFER_FLUSH_INTERVAL_SECS: u64 = 10;

/// Bounded capacity of the log channel. Beyond this depth (a sustained burst
/// the database writer cannot drain) entries are dropped rather than queued, so
/// a logging backlog cannot grow the heap without bound.
const CHANNEL_CAPACITY: usize = 8192;

static BACKGROUND_SENDER: OnceLock<mpsc::Sender<LogCommand>> = OnceLock::new();
static BACKGROUND_DROPPED: AtomicU64 = AtomicU64::new(0);

/// Non-blocking and off the caller's hot path: the entry is dropped (and
/// counted) if the sink is unattached or the channel is full. Error entries also
/// request an immediate flush.
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
        sender.try_send(LogCommand::FlushNow).ok();
    }
}

enum LogCommand {
    Entry(Box<LogEntry>),
    FlushNow,
}

/// Bounded sender to the database writer task. On a full channel the entry is
/// dropped and [`LogChannel::dropped`] is incremented; the send never blocks,
/// so logging stays off the hot path even under burst.
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
    pub fn new(db_pool: DbPool) -> Self {
        let (channel, receiver) = LogChannel::new(CHANNEL_CAPACITY);

        BACKGROUND_SENDER.get_or_init(|| channel.sender.clone());

        tokio::spawn(Self::batch_writer(db_pool, receiver));

        Self { channel }
    }

    async fn batch_writer(db_pool: DbPool, mut receiver: mpsc::Receiver<LogCommand>) {
        let mut buffer = Vec::with_capacity(BUFFER_FLUSH_SIZE);
        let mut interval = tokio::time::interval(Duration::from_secs(BUFFER_FLUSH_INTERVAL_SECS));
        let mut failed_total: u64 = 0;

        loop {
            tokio::select! {
                Some(command) = receiver.recv() => {
                    match command {
                        LogCommand::Entry(entry) => {
                            buffer.push(*entry);
                            if buffer.len() >= BUFFER_FLUSH_SIZE {
                                Self::flush(&db_pool, &mut buffer, &mut failed_total).await;
                            }
                        }
                        LogCommand::FlushNow => {
                            if !buffer.is_empty() {
                                Self::flush(&db_pool, &mut buffer, &mut failed_total).await;
                            }
                        }
                    }
                }
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        Self::flush(&db_pool, &mut buffer, &mut failed_total).await;
                    }
                }
            }
        }
    }

    async fn flush(db_pool: &DbPool, buffer: &mut Vec<LogEntry>, failed_total: &mut u64) {
        if let Err(e) = Self::batch_insert(db_pool, buffer).await {
            let lost = u64::try_from(buffer.len()).unwrap_or(u64::MAX);
            *failed_total = failed_total.saturating_add(lost);
            writeln!(
                std::io::stderr(),
                "DATABASE LOG FLUSH FAILED ({lost} entries lost this flush, {failed_total} total lost since start): {e}"
            )
            .ok();
        }
        buffer.clear();
    }

    async fn batch_insert(
        db_pool: &DbPool,
        entries: &[LogEntry],
    ) -> Result<(), crate::models::LoggingError> {
        let pool = db_pool.write_pool_arc()?;

        // One commit per flush, fsync off: the audit log is best-effort, so a
        // few buffered rows lost on an unclean shutdown is an acceptable trade.
        let mut tx = pool.begin().await?;
        sqlx::query!("SET LOCAL synchronous_commit = off")
            .execute(&mut *tx)
            .await?;

        for entry in entries {
            let metadata_json: Option<String> = entry
                .metadata
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?;

            let entry_id = entry.id.as_str();
            let level_str = entry.level.to_string();
            let user_id = entry.user_id.as_str();
            let session_id = entry.session_id.as_str();
            let task_id = entry.task_id.as_ref().map(TaskId::as_str);
            let trace_id = entry.trace_id.as_str();
            let context_id = entry.context_id.as_ref().map(ContextId::as_str);
            let client_id = entry.client_id.as_ref().map(ClientId::as_str);

            sqlx::query!(
                r"
                INSERT INTO logs (id, timestamp, level, module, message, metadata, user_id, session_id, task_id, trace_id, context_id, client_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                ",
                entry_id,
                entry.timestamp,
                level_str,
                entry.module,
                entry.message,
                metadata_json,
                user_id,
                session_id,
                task_id,
                trace_id,
                context_id,
                client_id
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
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
