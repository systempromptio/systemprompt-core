//! `tracing` subscriber layer that persists events to the database.
//!
//! [`DatabaseLayer`] buffers log events off the hot path and batch-inserts them
//! from a background task, flushing on a size threshold, a timer, or
//! immediately on an error. [`ProxyDatabaseLayer`] is the proxy-side variant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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

enum LogCommand {
    Entry(Box<LogEntry>),
    FlushNow,
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
            // Why: stderr is the last-resort sink once the database layer has
            // failed; tracing here would feed back into the failing layer.
            let _stderr_written = writeln!(
                std::io::stderr(),
                "DATABASE LOG FLUSH FAILED ({lost} entries lost this flush, {failed_total} total lost since start): {e}"
            )
            .is_ok();
        }
        buffer.clear();
    }

    async fn batch_insert(
        db_pool: &DbPool,
        entries: &[LogEntry],
    ) -> Result<(), crate::models::LoggingError> {
        let pool = db_pool.write_pool_arc()?;

        let mut tx = pool.begin().await?;
        sqlx::query!("SET LOCAL synchronous_commit = off")
            .execute(&mut *tx)
            .await?;

        Self::insert_batch(&mut tx, entries).await?;
        tx.commit().await?;
        Ok(())
    }

    /// One INSERT per flush over parallel column arrays.
    async fn insert_batch(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        entries: &[LogEntry],
    ) -> Result<(), crate::models::LoggingError> {
        // Why: one INSERT per flush. The logs table carries a statement-level
        // reporting capture trigger, so a hundred rows cost one outbox write
        // here and a hundred with a row-per-INSERT loop.
        let columns = LogColumns::gather(entries)?;
        // Why: sqlx infers `&[String]` for a text[] bind; the nullable
        // columns need their element type stated once, without a cast.
        let metadata: &[Option<String>] = &columns.metadata;
        let task_ids: &[Option<String>] = &columns.task_ids;
        let context_ids: &[Option<String>] = &columns.context_ids;
        let client_ids: &[Option<String>] = &columns.client_ids;
        let instance_ids: &[Option<String>] = &columns.instance_ids;
        sqlx::query!(
            r"
            INSERT INTO logs (id, timestamp, level, module, message, metadata, user_id, session_id, task_id, trace_id, context_id, client_id, instance_id)
            SELECT * FROM UNNEST($1::text[], $2::timestamptz[], $3::text[], $4::text[], $5::text[], $6::text[], $7::text[], $8::text[], $9::text[], $10::text[], $11::text[], $12::text[], $13::text[])
            ",
            &columns.ids,
            &columns.timestamps,
            &columns.levels,
            &columns.modules,
            &columns.messages,
            metadata as _,
            &columns.user_ids,
            &columns.session_ids,
            task_ids as _,
            &columns.trace_ids,
            context_ids as _,
            client_ids as _,
            instance_ids as _
        )
        .execute(&mut **tx)
        .await?;

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

/// `logs` rows as parallel column arrays, the shape `UNNEST` binds.
struct LogColumns {
    ids: Vec<String>,
    timestamps: Vec<chrono::DateTime<chrono::Utc>>,
    levels: Vec<String>,
    modules: Vec<String>,
    messages: Vec<String>,
    metadata: Vec<Option<String>>,
    user_ids: Vec<String>,
    session_ids: Vec<String>,
    task_ids: Vec<Option<String>>,
    trace_ids: Vec<String>,
    context_ids: Vec<Option<String>>,
    client_ids: Vec<Option<String>>,
    instance_ids: Vec<Option<String>>,
}

impl LogColumns {
    fn gather(entries: &[LogEntry]) -> Result<Self, crate::models::LoggingError> {
        let mut ids = Vec::with_capacity(entries.len());
        let mut timestamps = Vec::with_capacity(entries.len());
        let mut levels = Vec::with_capacity(entries.len());
        let mut modules = Vec::with_capacity(entries.len());
        let mut messages = Vec::with_capacity(entries.len());
        let mut metadata = Vec::with_capacity(entries.len());
        let mut user_ids = Vec::with_capacity(entries.len());
        let mut session_ids = Vec::with_capacity(entries.len());
        let mut task_ids = Vec::with_capacity(entries.len());
        let mut trace_ids = Vec::with_capacity(entries.len());
        let mut context_ids = Vec::with_capacity(entries.len());
        let mut client_ids = Vec::with_capacity(entries.len());
        let mut instance_ids = Vec::with_capacity(entries.len());
        for entry in entries {
            ids.push(entry.id.as_str().to_owned());
            timestamps.push(entry.timestamp);
            levels.push(entry.level.to_string());
            modules.push(entry.module.clone());
            messages.push(entry.message.clone());
            metadata.push(
                entry
                    .metadata
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
            );
            user_ids.push(entry.user_id.as_str().to_owned());
            session_ids.push(entry.session_id.as_str().to_owned());
            task_ids.push(
                entry
                    .task_id
                    .as_ref()
                    .map(TaskId::as_str)
                    .map(str::to_owned),
            );
            trace_ids.push(entry.trace_id.as_str().to_owned());
            context_ids.push(
                entry
                    .context_id
                    .as_ref()
                    .map(ContextId::as_str)
                    .map(str::to_owned),
            );
            client_ids.push(
                entry
                    .client_id
                    .as_ref()
                    .map(ClientId::as_str)
                    .map(str::to_owned),
            );
            instance_ids.push(
                entry
                    .instance_id
                    .as_ref()
                    .map(systemprompt_identifiers::InstanceId::as_str)
                    .map(str::to_owned),
            );
        }
        Ok(Self {
            ids,
            timestamps,
            levels,
            modules,
            messages,
            metadata,
            user_ids,
            session_ids,
            task_ids,
            trace_ids,
            context_ids,
            client_ids,
            instance_ids,
        })
    }
}
