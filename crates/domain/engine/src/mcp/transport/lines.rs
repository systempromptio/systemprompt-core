//! Newline-delimited JSON-RPC over any byte stream: a child's stdio, or an
//! in-process pipe.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use super::{InboundSink, Transport};
use crate::error::McpError;

pub(crate) struct LineWriter<W> {
    inner: Mutex<Option<W>>,
}

impl<W: AsyncWrite + Unpin + Send> LineWriter<W> {
    pub(crate) fn new(writer: W) -> Self {
        Self {
            inner: Mutex::new(Some(writer)),
        }
    }

    // JSON: one complete JSON-RPC message, written as a single line.
    pub(crate) async fn write(&self, message: &Value) -> Result<(), McpError> {
        let mut line = serde_json::to_vec(message).map_err(McpError::Json)?;
        line.push(b'\n');
        let mut guard = self.inner.lock().await;
        let writer = guard.as_mut().ok_or(McpError::Closed)?;
        writer.write_all(&line).await.map_err(McpError::Io)?;
        let flushed = writer.flush().await.map_err(McpError::Io);
        drop(guard);
        flushed
    }

    pub(crate) async fn shutdown(&self) {
        let writer = self.inner.lock().await.take();
        if let Some(mut writer) = writer
            && let Err(error) = writer.shutdown().await
        {
            tracing::debug!(error = %error, "closing the server's input failed");
        }
    }
}

pub(crate) fn spawn_line_reader<R>(reader: R, sink: InboundSink)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let tasks = sink.tasks().clone();
    tasks.spawn("mcp-line-reader", read_lines(reader, sink));
}

async fn read_lines<R: AsyncRead + Unpin>(reader: R, sink: InboundSink) {
    let closed = sink.closed_token();
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = tokio::select! {
            () = closed.cancelled() => break,
            read = reader.read_until(b'\n', &mut line) => read,
        };
        match read {
            Ok(0) => break,
            Ok(_) => deliver_line(&sink, &line),
            Err(error) => {
                tracing::debug!(error = %error, "reading from the server failed");
                break;
            },
        }
    }
    sink.close();
}

fn deliver_line(sink: &InboundSink, line: &[u8]) {
    let text = line.trim_ascii();
    if text.is_empty() {
        return;
    }
    match serde_json::from_slice::<Value>(text) {
        Ok(message) => sink.deliver(message),
        Err(error) => tracing::debug!(error = %error, "ignoring a non-JSON line from the server"),
    }
}

pub(crate) struct StreamTransport<W> {
    writer: LineWriter<W>,
    sink: InboundSink,
}

impl<W: AsyncWrite + Unpin + Send> StreamTransport<W> {
    pub(crate) fn new(writer: W, sink: InboundSink) -> Self {
        Self {
            writer: LineWriter::new(writer),
            sink,
        }
    }
}

#[async_trait::async_trait]
impl<W: AsyncWrite + Unpin + Send + 'static> Transport for StreamTransport<W> {
    // JSON: one complete JSON-RPC message, written as a single line.
    async fn send(&self, message: Value) -> Result<(), McpError> {
        self.writer.write(&message).await
    }

    async fn close(&self) {
        self.writer.shutdown().await;
        self.sink.close();
    }
}
