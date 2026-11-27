//! The shared state of one server connection: outgoing frames, pending requests
//! and liveness.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::time::Duration;

use systemprompt_traits::CancellationToken;
use parking_lot::Mutex;
use serde_json::{Map, Value, json};
use tokio::sync::{mpsc, oneshot, watch};
use tracing::{debug, warn};

use crate::lsp::error::LspError;
use crate::lsp::framing::encode_frame;
use crate::lsp::spec::ServerId;
use crate::lsp::types::Diagnostic;

#[derive(Debug)]
pub(super) struct RemoteError {
    pub(super) code: i64,
    pub(super) message: String,
}

pub(super) type Reply = Result<Value, RemoteError>;

#[derive(Debug)]
pub(super) struct Published {
    pub(super) diagnostics: Vec<Diagnostic>,
    pub(super) version: Option<i64>,
    pub(super) seq: u64,
}

pub(super) struct Shared {
    pub(super) server: ServerId,
    pub(super) root: PathBuf,
    // JSON: the server's configured settings, answered to `workspace/configuration`.
    pub(super) settings: Option<Value>,
    pub(super) outgoing: mpsc::UnboundedSender<Vec<u8>>,
    pub(super) pending: Mutex<HashMap<i64, oneshot::Sender<Reply>>>,
    pub(super) next_id: AtomicI64,
    pub(super) alive: AtomicBool,
    pub(super) failure: Mutex<Option<String>>,
    pub(super) stderr_tail: Mutex<VecDeque<String>>,
    pub(super) published: Mutex<HashMap<PathBuf, Published>>,
    pub(super) publish_seq: AtomicU64,
    pub(super) publish_events: watch::Sender<u64>,
    pub(super) cancel: CancellationToken,
}

impl fmt::Debug for Shared {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shared")
            .field("server", &self.server)
            .field("root", &self.root)
            .field("alive", &self.alive.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

impl Shared {
    pub(super) fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    fn closed(&self) -> LspError {
        LspError::Closed {
            server: self.server.clone(),
        }
    }

    // JSON: a whole JSON-RPC message.
    pub(super) fn send(&self, message: &Value) -> Result<(), LspError> {
        if !self.is_alive() {
            return Err(self.closed());
        }
        let body = serde_json::to_vec(message).map_err(|source| LspError::Encode {
            server: self.server.clone(),
            source,
        })?;
        self.outgoing.send(encode_frame(&body)).map_err(|error| {
            debug!(server = %self.server, %error, "the writer task has stopped");
            self.closed()
        })
    }

    // JSON: protocol params in, a JSON-RPC message without an id out.
    fn message(method: &str, params: Value) -> Map<String, Value> {
        let mut message = Map::new();
        message.insert("jsonrpc".to_owned(), Value::from("2.0"));
        message.insert("method".to_owned(), Value::from(method));
        if !params.is_null() {
            message.insert("params".to_owned(), params);
        }
        message
    }

    // JSON: the params of a protocol notification.
    pub(super) fn notify(&self, method: &str, params: Value) -> Result<(), LspError> {
        self.send(&Value::Object(Self::message(method, params)))
    }

    // JSON: protocol request params in, the raw protocol result out.
    pub(super) async fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, LspError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (reply_tx, reply_rx) = oneshot::channel();
        self.pending.lock().insert(id, reply_tx);
        let mut message = Self::message(method, params);
        message.insert("id".to_owned(), Value::from(id));
        if let Err(error) = self.send(&Value::Object(message)) {
            self.pending.lock().remove(&id);
            return Err(error);
        }
        match tokio::time::timeout(timeout, reply_rx).await {
            Err(_) => {
                self.pending.lock().remove(&id);
                if let Err(error) = self.notify("$/cancelRequest", json!({ "id": id })) {
                    debug!(server = %self.server, %error, "could not cancel a timed-out request");
                }
                Err(LspError::Timeout {
                    server: self.server.clone(),
                    method: method.to_owned(),
                    timeout_ms: timeout.as_millis(),
                })
            },
            Ok(Err(_)) => Err(self.closed()),
            Ok(Ok(Err(remote))) => Err(LspError::Remote {
                server: self.server.clone(),
                method: method.to_owned(),
                code: remote.code,
                message: remote.message,
            }),
            Ok(Ok(Ok(result))) => Ok(result),
        }
    }

    pub(super) fn mark_dead(&self, reason: &str) {
        if !self.alive.swap(false, Ordering::SeqCst) {
            return;
        }
        let detail = self
            .stderr_tail
            .lock()
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .cloned();
        let reason = detail.map_or_else(|| reason.to_owned(), |line| format!("{reason}: {line}"));
        warn!(server = %self.server, root = %self.root.display(), %reason, "language server stopped");
        *self.failure.lock() = Some(reason);
        self.finish();
    }

    pub(super) fn mark_stopped(&self) {
        self.alive.store(false, Ordering::SeqCst);
        self.finish();
    }

    fn finish(&self) {
        self.pending.lock().clear();
        self.publish_events
            .send_modify(|value| *value = value.wrapping_add(1));
        self.cancel.cancel();
    }
}
