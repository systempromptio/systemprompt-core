//! The background task that batch-inserts buffered log entries, and the
//! [`LogWriterHandle`] its owner awaits to flush and stop it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Write;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::columns::LogColumns;
use crate::models::{LogEntry, LoggingError};
use systemprompt_database::DbPool;

const BUFFER_FLUSH_SIZE: usize = 100;
const BUFFER_FLUSH_INTERVAL_SECS: u64 = 10;

pub(super) enum LogCommand {
    Entry(Box<LogEntry>),
    FlushNow,
    Shutdown,
}

#[derive(Debug, thiserror::Error)]
pub enum LogWriterShutdownError {
    #[error("the log writer had already stopped before the shutdown flush was requested")]
    WriterGone,

    #[error("the log writer task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
}

#[derive(Debug)]
pub struct LogWriterHandle {
    sender: mpsc::Sender<LogCommand>,
    task: JoinHandle<()>,
}

impl LogWriterHandle {
    pub(super) fn spawn(
        db_pool: DbPool,
        sender: mpsc::Sender<LogCommand>,
        receiver: mpsc::Receiver<LogCommand>,
    ) -> Self {
        Self {
            sender,
            task: tokio::spawn(batch_writer(db_pool, receiver)),
        }
    }

    pub async fn shutdown(self) -> Result<(), LogWriterShutdownError> {
        let requested = self.sender.send(LogCommand::Shutdown).await;
        self.task.await?;
        requested.map_err(|_closed| LogWriterShutdownError::WriterGone)
    }
}

async fn batch_writer(db_pool: DbPool, mut receiver: mpsc::Receiver<LogCommand>) {
    let mut buffer = Vec::with_capacity(BUFFER_FLUSH_SIZE);
    let mut interval = tokio::time::interval(Duration::from_secs(BUFFER_FLUSH_INTERVAL_SECS));
    let mut failed_total: u64 = 0;

    loop {
        tokio::select! {
            command = receiver.recv() => match command {
                Some(LogCommand::Entry(entry)) => {
                    buffer.push(*entry);
                    if buffer.len() >= BUFFER_FLUSH_SIZE {
                        flush(&db_pool, &mut buffer, &mut failed_total).await;
                    }
                }
                Some(LogCommand::FlushNow) => {
                    if !buffer.is_empty() {
                        flush(&db_pool, &mut buffer, &mut failed_total).await;
                    }
                }
                Some(LogCommand::Shutdown) | None => {
                    drain_queued(&mut receiver, &mut buffer);
                    if !buffer.is_empty() {
                        flush(&db_pool, &mut buffer, &mut failed_total).await;
                    }
                    return;
                }
            },
            _ = interval.tick() => {
                if !buffer.is_empty() {
                    flush(&db_pool, &mut buffer, &mut failed_total).await;
                }
            }
        }
    }
}

fn drain_queued(receiver: &mut mpsc::Receiver<LogCommand>, buffer: &mut Vec<LogEntry>) {
    while let Ok(command) = receiver.try_recv() {
        if let LogCommand::Entry(entry) = command {
            buffer.push(*entry);
        }
    }
}

async fn flush(db_pool: &DbPool, buffer: &mut Vec<LogEntry>, failed_total: &mut u64) {
    if let Err(e) = batch_insert(db_pool, buffer).await {
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

async fn batch_insert(db_pool: &DbPool, entries: &[LogEntry]) -> Result<(), LoggingError> {
    let pool = db_pool.write_pool();

    let mut tx = pool.begin().await?;
    sqlx::query!("SET LOCAL synchronous_commit = off")
        .execute(&mut *tx)
        .await?;

    insert_batch(&mut tx, entries).await?;
    tx.commit().await?;
    Ok(())
}

async fn insert_batch(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entries: &[LogEntry],
) -> Result<(), LoggingError> {
    // Why: one INSERT per flush — a hundred rows cost one round trip
    // and one statement instead of a hundred.
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
