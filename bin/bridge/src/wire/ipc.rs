//! IPC message and reply payload types exchanged between the GUI webview and
//! the bridge.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
#[serde(rename_all = "snake_case")]
pub enum ErrorScope {
    Gateway,
    Identity,
    Marketplace,
    Host,
    Proxy,
    Internal,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unreachable,
    Unauthorized,
    InvalidArgs,
    InvalidFormat,
    NotFound,
    Conflict,
    Timeout,
    ElevationRequired,
    Partial,
    Internal,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct BridgeError {
    pub scope: ErrorScope,
    pub code: ErrorCode,
    pub message: String,
    // JSON: webview IPC envelope, free-form diagnostic detail shown verbatim
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional, type = "unknown"))]
    pub detail: Option<Value>,
}

impl BridgeError {
    pub fn new(scope: ErrorScope, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            scope,
            code,
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: Value) -> Self {
        self.detail = Some(detail);
        self
    }

    pub fn invalid_args(message: impl Into<String>) -> Self {
        Self::new(ErrorScope::Internal, ErrorCode::InvalidArgs, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorScope::Internal, ErrorCode::NotFound, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorScope::Internal, ErrorCode::Internal, message)
    }
}

/// Which webview mount a request came from, and which reply it awaits.
///
/// The mount nonce is minted by the bootstrap script on every page load so a
/// reply produced for a previous mount can be recognised and dropped on both
/// sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReplyTarget {
    pub mount: u64,
    pub id: u64,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct IpcRequest {
    pub id: u64,
    pub mount: u64,
    pub cmd: String,
    // JSON: webview IPC envelope, args typed per command at parse()
    #[serde(default)]
    #[cfg_attr(feature = "ts-export", ts(type = "unknown"))]
    pub args: Value,
}

impl IpcRequest {
    #[must_use]
    pub const fn reply_target(&self) -> ReplyTarget {
        ReplyTarget {
            mount: self.mount,
            id: self.id,
        }
    }
}

/// The part of a request envelope that can still be read when the whole
/// cannot: enough to address a rejection back at the promise that is waiting.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct IpcEnvelopeHead {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub mount: Option<u64>,
}

impl IpcEnvelopeHead {
    #[must_use]
    pub fn of(raw: &str) -> Option<Self> {
        serde_json::from_str(raw).ok()
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct IpcReplyPayload {
    pub ok: bool,
    // JSON: webview IPC envelope, the command's typed reply serialized at emit
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional, type = "unknown"))]
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub error: Option<BridgeError>,
}

impl IpcReplyPayload {
    pub const fn ok(value: Value) -> Self {
        Self {
            ok: true,
            value: Some(value),
            error: None,
        }
    }

    pub const fn err(error: BridgeError) -> Self {
        Self {
            ok: false,
            value: None,
            error: Some(error),
        }
    }
}

pub fn reply_script(target: ReplyTarget, payload: &IpcReplyPayload) -> String {
    let body = serde_json::to_string(payload)
        .unwrap_or_else(|_| r#"{"ok":false,"error":{"scope":"internal","code":"internal","message":"reply encode failed"}}"#.to_owned());
    let ReplyTarget { mount, id } = target;
    format!(
        "window.__bridge && window.__bridge.reply && window.__bridge.reply({mount}, {id}, {body});"
    )
}

pub fn emit_script(channel: &str, payload: &Value) -> String {
    let channel_json = serde_json::to_string(channel).unwrap_or_else(|_| "\"unknown\"".to_owned());
    let body = serde_json::to_string(payload).unwrap_or_else(|_| "null".to_owned());
    format!(
        "window.__bridge && window.__bridge.emit && window.__bridge.emit({channel_json}, {body});"
    )
}
