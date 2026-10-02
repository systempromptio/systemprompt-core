//! The background task that batch-inserts buffered log entries, and the
//! [`LogWriterHandle`] its owner awaits to flush and stop it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Write;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::models::LogEntry;
use crate::repository::LoggingRepository;

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
        repository: LoggingRepository,
        sender: mpsc::Sender<LogCommand>,
        receiver: mpsc::Receiver<LogCommand>,
    ) -> Self {
        Self {
            sender,
            task: tokio::spawn(batch_writer(repository, receiver)),
        }
    }

    pub async fn shutdown(self) -> Result<(), LogWriterShutdownError> {
        let requested = self.sender.send(LogCommand::Shutdown).await;
        self.task.await?;
        requested.map_err(|_closed| LogWriterShutdownError::WriterGone)
    }
}

async fn batch_writer(repository: LoggingRepository, mut receiver: mpsc::Receiver<LogCommand>) {
    let mut buffer = Vec::with_capacity(BUFFER_FLUSH_SIZE);
    let mut interval = tokio::time::interval(Duration::from_secs(BUFFER_FLUSH_INTERVAL_SECS));
    let mut failed_total: u64 = 0;

    loop {
        tokio::select! {
            command = receiver.recv() => match command {
                Some(LogCommand::Entry(entry)) => {
                    buffer.push(*entry);
                    if buffer.len() >= BUFFER_FLUSH_SIZE {
                        flush(&repository, &mut buffer, &mut failed_total).await;
                    }
                }
                Some(LogCommand::FlushNow) => {
                    if !buffer.is_empty() {
                        flush(&repository, &mut buffer, &mut failed_total).await;
                    }
                }
                Some(LogCommand::Shutdown) | None => {
                    drain_queued(&mut receiver, &mut buffer);
                    if !buffer.is_empty() {
                        flush(&repository, &mut buffer, &mut failed_total).await;
                    }
                    return;
                }
            },
            _ = interval.tick() => {
                if !buffer.is_empty() {
                    flush(&repository, &mut buffer, &mut failed_total).await;
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

async fn flush(
    repository: &LoggingRepository,
    buffer: &mut Vec<LogEntry>,
    failed_total: &mut u64,
) {
    if let Err(e) = repository.insert_batch(buffer).await {
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
