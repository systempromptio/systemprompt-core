use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorScope {
    Gateway,
    Identity,
    Marketplace,
    Host,
    Proxy,
    Setup,
    Internal,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unreachable,
    Unauthorized,
    InvalidArgs,
    InvalidFormat,
    NotFound,
    Conflict,
    Timeout,
    Internal,
}

#[derive(Debug, Clone, Serialize)]
pub struct BridgeError {
    pub scope: ErrorScope,
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
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

#[derive(Debug, Deserialize)]
pub struct IpcRequest {
    pub id: u64,
    pub cmd: String,
    #[serde(default)]
    pub args: Value,
}

#[derive(Debug, Serialize)]
pub struct IpcReplyPayload {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<BridgeError>,
}

impl IpcReplyPayload {
    pub fn ok(value: Value) -> Self {
        Self {
            ok: true,
            value: Some(value),
            error: None,
        }
    }

    pub fn err(error: BridgeError) -> Self {
        Self {
            ok: false,
            value: None,
            error: Some(error),
        }
    }
}

pub fn reply_script(id: u64, payload: &IpcReplyPayload) -> String {
    let body = serde_json::to_string(payload)
        .unwrap_or_else(|_| r#"{"ok":false,"error":{"scope":"internal","code":"internal","message":"reply encode failed"}}"#.to_string());
    format!("window.__bridge && window.__bridge.reply && window.__bridge.reply({id}, {body});")
}

pub fn emit_script(channel: &str, payload: &Value) -> String {
    let channel_json =
        serde_json::to_string(channel).unwrap_or_else(|_| "\"unknown\"".to_string());
    let body = serde_json::to_string(payload).unwrap_or_else(|_| "null".to_string());
    format!("window.__bridge && window.__bridge.emit && window.__bridge.emit({channel_json}, {body});")
}
