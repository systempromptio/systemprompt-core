//! Pumps the child's stdout and stderr into the logs and the [`EventSink`].
//!
//! Every stdout line is appended verbatim to `logs/stdout.jsonl`, decoded by
//! the host adapter and numbered into [`RunEvent`]s that [`BufferedSink`]
//! hands to the sink in batches (50 events or 500 ms, whichever comes
//! first). The first sink failure is kept and later emits are skipped, so
//! the pump keeps draining the child instead of stalling it on a full pipe;
//! the failure is returned once the run has ended. Stderr goes to
//! `logs/stderr.log`; its last lines are kept for the failure message and the
//! trailing `stderr` events.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;

use chrono::Utc;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::sink::EventSink;
use crate::error::{AgentRunsError, AgentRunsResult};
use crate::event::{RunEvent, RunEventPayload};
use crate::host::HeadlessHost;

pub(super) const BATCH_SIZE: usize = 50;
pub(super) const BATCH_INTERVAL: Duration = Duration::from_millis(500);
pub(super) const STDERR_TAIL_LINES: usize = 50;
pub(super) const STDERR_EVENT_LINES: usize = 20;

pub(super) struct BufferedSink<'a> {
    sink: &'a dyn EventSink,
    next_seq: i64,
    buffer: Vec<RunEvent>,
    last_flush: Instant,
    session_seen: bool,
    pub payloads: Vec<RunEventPayload>,
    failure: Option<AgentRunsError>,
}

impl std::fmt::Debug for BufferedSink<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferedSink")
            .field("next_seq", &self.next_seq)
            .field("buffered", &self.buffer.len())
            .finish_non_exhaustive()
    }
}

impl<'a> BufferedSink<'a> {
    #[must_use]
    pub(super) fn new(sink: &'a dyn EventSink, seq_after: Option<i64>) -> Self {
        Self {
            sink,
            next_seq: seq_after.map_or(0, |seq| seq + 1),
            buffer: Vec::new(),
            last_flush: Instant::now(),
            session_seen: false,
            payloads: Vec::new(),
            failure: None,
        }
    }

    pub(super) async fn observe_session(
        &mut self,
        session_id: &systemprompt_identifiers::HostSessionId,
    ) {
        if self.session_seen || self.failure.is_some() {
            return;
        }
        self.session_seen = true;
        if let Err(error) = self.sink.session_observed(session_id).await {
            self.failure = Some(error);
        }
    }

    pub(super) async fn push(&mut self, payload: RunEventPayload) {
        if let RunEventPayload::Init { session_id, .. } = &payload {
            let session_id = session_id.clone();
            self.observe_session(&session_id).await;
        }
        let tool_name = match &payload {
            RunEventPayload::ToolUse { name, .. } => Some(name.clone()),
            _ => None,
        };
        self.buffer.push(RunEvent {
            seq: self.next_seq,
            kind: payload.kind(),
            tool_name,
            tool_use_id: payload.tool_use_id().map(str::to_owned),
            payload: payload.clone(),
            created_at: Utc::now(),
        });
        self.next_seq += 1;
        self.payloads.push(payload);
    }

    pub(super) async fn flush(&mut self) {
        self.last_flush = Instant::now();
        if self.buffer.is_empty() {
            return;
        }
        let batch = std::mem::take(&mut self.buffer);
        if self.failure.is_some() {
            return;
        }
        if let Err(error) = self.sink.emit(batch).await {
            self.failure = Some(error);
        }
    }

    pub(super) async fn flush_if_due(&mut self) {
        if self.buffer.len() >= BATCH_SIZE || self.last_flush.elapsed() >= BATCH_INTERVAL {
            self.flush().await;
        }
    }

    pub(super) fn finish(self) -> AgentRunsResult<Vec<RunEventPayload>> {
        self.failure.map_or(Ok(self.payloads), Err)
    }
}

pub(super) async fn pump_stdout<R: AsyncRead + Unpin>(
    stdout: R,
    log_path: &Path,
    host: &dyn HeadlessHost,
    sink: &mut BufferedSink<'_>,
    stop: &CancellationToken,
) {
    let mut log = open_log(log_path).await;
    let mut lines = BufReader::new(stdout).lines();
    let mut tick = tokio::time::interval(BATCH_INTERVAL);
    loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    append_line(&mut log, &line).await;
                    for event in host.parse_line(&line) {
                        if let Some(session_id) = &event.session_id {
                            sink.observe_session(session_id).await;
                        }
                        for payload in event.payloads {
                            sink.push(payload).await;
                        }
                    }
                    sink.flush_if_due().await;
                },
                Ok(None) => break,
                Err(error) => {
                    tracing::warn!(%error, "headless stdout read failed");
                    break;
                },
            },
            _ = tick.tick() => sink.flush_if_due().await,
            () = stop.cancelled() => break,
        }
    }
    close_log(log).await;
    sink.flush().await;
}

pub(super) async fn pump_stderr<R: AsyncRead + Unpin>(
    stderr: R,
    log_path: &Path,
    stop: &CancellationToken,
) -> Vec<String> {
    let mut log = open_log(log_path).await;
    let mut lines = BufReader::new(stderr).lines();
    let mut tail: VecDeque<String> = VecDeque::with_capacity(STDERR_TAIL_LINES);
    loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    append_line(&mut log, &line).await;
                    if tail.len() == STDERR_TAIL_LINES {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                },
                Ok(None) => break,
                Err(error) => {
                    tracing::warn!(%error, "headless stderr read failed");
                    break;
                },
            },
            () = stop.cancelled() => break,
        }
    }
    close_log(log).await;
    tail.into()
}

async fn open_log(path: &Path) -> Option<tokio::fs::File> {
    match tokio::fs::File::create(path).await {
        Ok(file) => Some(file),
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "headless run log open failed");
            None
        },
    }
}

async fn append_line(log: &mut Option<tokio::fs::File>, line: &str) {
    let Some(file) = log.as_mut() else {
        return;
    };
    let mut bytes = Vec::with_capacity(line.len() + 1);
    bytes.extend_from_slice(line.as_bytes());
    bytes.push(b'\n');
    if let Err(error) = file.write_all(&bytes).await {
        tracing::warn!(%error, "headless run log write failed; logging stops");
        *log = None;
    }
}

async fn close_log(log: Option<tokio::fs::File>) {
    if let Some(mut file) = log
        && let Err(error) = file.flush().await
    {
        tracing::warn!(%error, "headless run log flush failed");
    }
}
