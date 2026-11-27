//! One server session's shared state: the transport, the requests awaiting a
//! reply, the notification feed and the background work that reads and answers
//! the server. The transport is held as `Box<dyn Transport>`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, Weak};
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{Map, Value, json};
use systemprompt_traits::BackgroundTasks;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use super::options::{DEFAULT_REQUEST_TIMEOUT, RequestOptions, ServerNotification};
use crate::mcp::error::McpError;
use crate::mcp::jsonrpc::{self, Incoming, METHOD_NOT_FOUND};
use crate::mcp::transport::{InboundSink, Transport};
use crate::mcp::types::ServerInfo;

const NOTIFICATION_BUFFER: usize = 64;
const TASK_DRAIN: Duration = Duration::from_secs(2);

type Reply = Result<Value, McpError>;

pub(super) struct Session {
    pub(super) transport: Box<dyn Transport>,
    pub(super) closed: CancellationToken,
    pub(super) notifications: broadcast::Sender<ServerNotification>,
    pub(super) server: OnceLock<ServerInfo>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Reply>>>,
    next_id: AtomicU64,
    tasks: BackgroundTasks,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.closed.cancel();
    }
}

struct PendingGuard<'a> {
    session: &'a Session,
    id: u64,
}

impl Drop for PendingGuard<'_> {
    fn drop(&mut self) {
        self.session.pending.lock().remove(&self.id);
    }
}

impl Session {
    // JSON: the inbound channel carries decoded JSON-RPC messages for the session.
    pub(super) fn start(
        transport: Box<dyn Transport>,
        sink: &InboundSink,
        inbound: mpsc::UnboundedReceiver<Value>,
    ) -> Arc<Self> {
        let session = Arc::new(Self {
            transport,
            closed: sink.closed_token(),
            notifications: broadcast::channel(NOTIFICATION_BUFFER).0,
            server: OnceLock::new(),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            tasks: sink.tasks().clone(),
        });
        let reader = read_inbound(Arc::downgrade(&session), inbound, sink.closed_token());
        session.tasks.spawn("mcp-session-reader", reader);
        session
    }

    pub(super) fn is_closed(&self) -> bool {
        self.closed.is_cancelled()
    }

    // JSON: request params and result are the called method's own JSON-RPC payload.
    pub(super) async fn request(
        self: &Arc<Self>,
        method: &str,
        params: Option<Value>,
        options: RequestOptions,
    ) -> Result<Value, McpError> {
        if self.is_closed() {
            return Err(McpError::Closed);
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (waiter, reply) = oneshot::channel();
        self.pending.lock().insert(id, waiter);
        let _guard = PendingGuard { session: self, id };
        let timeout = options.timeout.unwrap_or(DEFAULT_REQUEST_TIMEOUT);
        let cancel = options.cancel.unwrap_or_default();
        let message = jsonrpc::request(id, method, params);
        let exchange = async {
            self.transport.send(message).await?;
            reply.await.unwrap_or(Err(McpError::Closed))
        };
        let outcome = tokio::select! {
            biased;
            outcome = exchange => outcome,
            () = cancel.cancelled() => Err(McpError::Cancelled),
            () = self.closed.cancelled() => Err(McpError::Closed),
            () = tokio::time::sleep(timeout) => Err(McpError::Timeout(timeout)),
        };
        if let Err(error @ (McpError::Timeout(_) | McpError::Cancelled)) = &outcome {
            self.abandon(method, id, error);
        }
        outcome
    }

    pub(super) fn fail_pending(&self) {
        let waiting: Vec<oneshot::Sender<Reply>> = self
            .pending
            .lock()
            .drain()
            .map(|(_, waiter)| waiter)
            .collect();
        for waiter in waiting {
            if waiter.send(Err(McpError::Closed)).is_err() {
                tracing::trace!("a waiting request had already given up");
            }
        }
    }

    pub(super) async fn drain_tasks(&self) {
        if !self.tasks.shutdown_or_abort(TASK_DRAIN).await.is_drained() {
            tracing::debug!("session tasks outlived the shutdown and were aborted");
        }
    }

    fn abandon(self: &Arc<Self>, method: &str, id: u64, error: &McpError) {
        tracing::debug!(method, id, error = %error, "request abandoned");
        if method == "initialize" || self.is_closed() {
            return;
        }
        let reason = if matches!(error, McpError::Timeout(_)) {
            "timed out"
        } else {
            "cancelled"
        };
        let notice = jsonrpc::notification(
            "notifications/cancelled",
            Some(json!({"requestId": id, "reason": reason})),
        );
        self.send_in_background("mcp-cancel-notice", notice);
    }

    // JSON: one decoded JSON-RPC message from the server.
    fn dispatch(self: &Arc<Self>, message: Value) {
        match jsonrpc::classify(message) {
            Incoming::Response { id, outcome } => self.resolve(id, outcome),
            Incoming::Request { id, method } => self.answer(id, &method),
            Incoming::Notification { method, params } => {
                let note = ServerNotification { method, params };
                if self.notifications.send(note).is_err() {
                    tracing::trace!("no subscriber for a server notification");
                }
            },
            Incoming::Invalid => tracing::debug!("ignoring a malformed message from the server"),
        }
    }

    fn resolve(&self, id: u64, outcome: Reply) {
        let waiter = self.pending.lock().remove(&id);
        let Some(waiter) = waiter else {
            tracing::debug!(id, "response to a request no longer waiting");
            return;
        };
        if waiter.send(outcome).is_err() {
            tracing::debug!(
                id,
                "the request stopped waiting before its response arrived"
            );
        }
    }

    // JSON: a server request's id is any JSON value and is echoed back verbatim.
    fn answer(self: &Arc<Self>, id: Value, method: &str) {
        let reply = if method == "ping" {
            jsonrpc::result(id, Value::Object(Map::new()))
        } else {
            tracing::debug!(method = %method, "declining a server request");
            jsonrpc::error(id, METHOD_NOT_FOUND, "method not supported by this client")
        };
        self.send_in_background("mcp-server-reply", reply);
    }

    // JSON: a complete JSON-RPC message sent without holding up the caller.
    fn send_in_background(self: &Arc<Self>, name: &'static str, message: Value) {
        let session = Arc::clone(self);
        self.tasks.spawn(name, async move {
            if let Err(error) = session.transport.send(message).await {
                tracing::debug!(task = name, error = %error, "background message to the server failed");
            }
        });
    }
}

// JSON: the inbound channel carries decoded JSON-RPC messages for the session.
async fn read_inbound(
    session: Weak<Session>,
    mut inbound: mpsc::UnboundedReceiver<Value>,
    closed: CancellationToken,
) {
    loop {
        let next = tokio::select! {
            biased;
            message = inbound.recv() => message,
            () = closed.cancelled() => None,
        };
        let Some(message) = next else { break };
        let Some(live) = session.upgrade() else {
            return;
        };
        live.dispatch(message);
    }
    closed.cancel();
    if let Some(live) = session.upgrade() {
        live.fail_pending();
    }
}
