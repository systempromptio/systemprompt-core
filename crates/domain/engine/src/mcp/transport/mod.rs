//! Transports carry JSON-RPC messages between the client and one server.
//! Outbound messages go through [`Transport::send`]; inbound ones arrive on the
//! channel behind an [`InboundSink`], in order. A session holds its transport
//! as `Box<dyn Transport>`, so the trait uses `async_trait`. The sink also
//! carries the session's background task set, so every task a transport starts
//! has the session as its owner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod http;
mod lines;
mod sse;
mod stdio;

use serde_json::Value;
use systemprompt_traits::BackgroundTasks;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(crate) use http::StreamableHttp;
pub(crate) use lines::{StreamTransport, spawn_line_reader};
pub(crate) use sse::LegacySse;
pub(crate) use stdio::{LocalCommand, StdioTransport};

use crate::mcp::error::McpError;

#[async_trait::async_trait]
pub(crate) trait Transport: Send + Sync {
    // JSON: one complete JSON-RPC message, framed by the transport.
    async fn send(&self, message: Value) -> Result<(), McpError>;

    fn set_protocol_version(&self, _version: &str) {}

    async fn close(&self);
}

#[derive(Debug, Clone)]
pub(crate) struct InboundSink {
    // JSON: decoded JSON-RPC messages on their way to the session.
    tx: mpsc::UnboundedSender<Value>,
    closed: CancellationToken,
    tasks: BackgroundTasks,
}

impl InboundSink {
    // JSON: decoded JSON-RPC messages on their way to the session.
    pub(crate) fn new() -> (Self, mpsc::UnboundedReceiver<Value>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Self {
                tx,
                closed: CancellationToken::new(),
                tasks: BackgroundTasks::new(),
            },
            rx,
        )
    }

    // JSON: a decoded JSON-RPC message or batch from the server.
    pub(crate) fn deliver(&self, message: Value) {
        match message {
            Value::Array(batch) => {
                for item in batch {
                    self.push(item);
                }
            },
            single => self.push(single),
        }
    }

    pub(crate) fn close(&self) {
        self.closed.cancel();
    }

    pub(crate) fn closed_token(&self) -> CancellationToken {
        self.closed.clone()
    }

    pub(crate) const fn tasks(&self) -> &BackgroundTasks {
        &self.tasks
    }

    // JSON: one decoded JSON-RPC message for the session.
    fn push(&self, message: Value) {
        if self.tx.send(message).is_err() {
            tracing::trace!("dropped a server message after the session ended");
        }
    }
}
