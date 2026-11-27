//! Messages from the server: responses to our requests, notifications, and
//! requests we answer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::Ordering;

use serde_json::{Map, Value, json};
use tracing::{debug, trace};

use super::connection::{Published, RemoteError, Shared};
use super::handshake::workspace_folder;
use crate::parse;

const METHOD_NOT_FOUND: i64 = -32601;

impl Shared {
    // JSON: one decoded JSON-RPC message from the server.
    pub(super) fn on_message(&self, message: &Value) {
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id").filter(|id| !id.is_null());
        match (method, id) {
            (Some(method), Some(id)) => self.on_request(method, id, message.get("params")),
            (Some(method), None) => self.on_notification(method, message.get("params")),
            (None, Some(id)) => self.on_response(id, message),
            (None, None) => {
                debug!(server = %self.server, "ignored a message with neither method nor id");
            },
        }
    }

    // JSON: the id and params of a JSON-RPC request from the server.
    fn on_request(&self, method: &str, id: &Value, params: Option<&Value>) {
        let mut reply = Map::new();
        reply.insert("jsonrpc".to_owned(), Value::from("2.0"));
        reply.insert("id".to_owned(), id.clone());
        match self.answer(method, params) {
            Ok(result) => {
                reply.insert("result".to_owned(), result);
            },
            Err((code, text)) => {
                reply.insert("error".to_owned(), json!({ "code": code, "message": text }));
            },
        }
        if let Err(error) = self.send(&Value::Object(reply)) {
            debug!(server = %self.server, %method, %error, "could not answer a server request");
        }
    }

    // JSON: the id and the whole JSON-RPC response.
    fn on_response(&self, id: &Value, message: &Value) {
        let Some(id) = id
            .as_i64()
            .or_else(|| id.as_str().and_then(|text| text.parse().ok()))
        else {
            return;
        };
        let Some(reply_tx) = self.pending.lock().remove(&id) else {
            return;
        };
        let reply = message
            .get("error")
            .filter(|error| !error.is_null())
            .map_or_else(
                || Ok(message.get("result").cloned().unwrap_or(Value::Null)),
                |error| {
                    Err(RemoteError {
                        code: error
                            .get("code")
                            .and_then(Value::as_i64)
                            .unwrap_or_default(),
                        message: error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    })
                },
            );
        if reply_tx.send(reply).is_err() {
            trace!(server = %self.server, id, "response arrived after the caller gave up");
        }
    }

    // JSON: the params of a protocol notification from the server.
    fn on_notification(&self, method: &str, params: Option<&Value>) {
        match method {
            "textDocument/publishDiagnostics" => {
                let Some(update) = params.and_then(parse::publish_diagnostics) else {
                    return;
                };
                let seq = self
                    .publish_seq
                    .fetch_add(1, Ordering::SeqCst)
                    .wrapping_add(1);
                trace!(
                    server = %self.server,
                    path = %update.path.display(),
                    count = update.diagnostics.len(),
                    "diagnostics published"
                );
                self.published.lock().insert(
                    update.path,
                    Published {
                        diagnostics: update.diagnostics,
                        version: update.version,
                        seq,
                    },
                );
                self.publish_events.send_replace(seq);
            },
            "window/logMessage" | "window/showMessage" => {
                let text = params
                    .and_then(|params| params.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                debug!(server = %self.server, message = %text, "server message");
            },
            _ => {},
        }
    }

    // JSON: the params of a protocol request from the server, and the result we
    // answer.
    fn answer(&self, method: &str, params: Option<&Value>) -> Result<Value, (i64, String)> {
        match method {
            "workspace/configuration" => {
                let items = params
                    .and_then(|params| params.get("items"))
                    .and_then(Value::as_array);
                let answers = items
                    .into_iter()
                    .flatten()
                    .map(|item| {
                        parse::configuration_section(
                            self.settings.as_ref(),
                            item.get("section").and_then(Value::as_str),
                        )
                    })
                    .collect();
                Ok(Value::Array(answers))
            },
            "workspace/workspaceFolders" => Ok(json!([workspace_folder(&self.root)])),
            "client/registerCapability"
            | "client/unregisterCapability"
            | "window/workDoneProgress/create"
            | "window/showMessageRequest"
            | "workspace/diagnostic/refresh"
            | "workspace/semanticTokens/refresh"
            | "workspace/inlayHint/refresh"
            | "workspace/codeLens/refresh" => Ok(Value::Null),
            "workspace/applyEdit" => {
                Ok(json!({ "applied": false, "failureReason": "this client does not apply edits" }))
            },
            "window/showDocument" => Ok(json!({ "success": false })),
            other => Err((METHOD_NOT_FOUND, format!("unsupported method {other}"))),
        }
    }
}
