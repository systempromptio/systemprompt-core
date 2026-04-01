mod proxy;
pub(crate) mod visitor;

use std::io::Write;
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

enum LogCommand {
    Entry(Box<LogEntry>),
    FlushNow,
}

pub struct DatabaseLayer {
    sender: mpsc::UnboundedSender<LogCommand>,
}

impl std::fmt::Debug for DatabaseLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseLayer").finish_non_exhaustive()
    }
}

impl DatabaseLayer {
    pub fn new(db_pool: DbPool) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        tokio::spawn(Self::batch_writer(db_pool, receiver));

        Self { sender }
    }

    async fn batch_writer(db_pool: DbPool, mut receiver: mpsc::UnboundedReceiver<LogCommand>) {
        let mut buffer = Vec::with_capacity(BUFFER_FLUSH_SIZE);
        let mut interval = tokio::time::interval(Duration::from_secs(BUFFER_FLUSH_INTERVAL_SECS));

        loop {
            tokio::select! {
                Some(command) = receiver.recv() => {
                    match command {
                        LogCommand::Entry(entry) => {
                            buffer.push(*entry);
                            if buffer.len() >= BUFFER_FLUSH_SIZE {
                                Self::flush(&db_pool, &mut buffer).await;
                            }
                        }
                        LogCommand::FlushNow => {
                            if !buffer.is_empty() {
                                Self::flush(&db_pool, &mut buffer).await;
                            }
                        }
                    }
                }
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        Self::flush(&db_pool, &mut buffer).await;
                    }
                }
            }
        }
    }

    async fn flush(db_pool: &DbPool, buffer: &mut Vec<LogEntry>) {
        if let Err(e) = Self::batch_insert(db_pool, buffer).await {
            let _ = writeln!(
                std::io::stderr(),
                "DATABASE LOG FLUSH FAILED ({} entries lost): {e}",
                buffer.len()
            );
        }
        buffer.clear();
    }

    async fn batch_insert(db_pool: &DbPool, entries: &[LogEntry]) -> anyhow::Result<()> {
        let pool = db_pool.write_pool_arc()?;
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
                INSERT INTO logs (id, level, module, message, metadata, user_id, session_id, task_id, trace_id, context_id, client_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ",
                entry_id,
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
            .execute(pool.as_ref())
            .await?;
        }

        Ok(())
    }
}

impl DatabaseLayer {
    fn send_entry(&self, entry: LogEntry) {
        let is_error = entry.level == LogLevel::Error;
        let _ = self.sender.send(LogCommand::Entry(Box::new(entry)));
        if is_error {
            let _ = self.sender.send(LogCommand::FlushNow);
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
        self.send_entry(build_log_entry(event, &ctx));
    }
}
