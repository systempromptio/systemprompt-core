//! A2A and JSON-RPC 2.0 error codes and the HTTP status each maps to in the
//! A2A JSON-RPC binding.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::StatusCode;

pub const TASK_NOT_FOUND: i32 = -32001;
pub const TASK_NOT_CANCELABLE: i32 = -32002;
pub const PUSH_NOTIFICATION_NOT_SUPPORTED: i32 = -32003;
pub const UNSUPPORTED_OPERATION: i32 = -32004;
pub const CONTENT_TYPE_NOT_SUPPORTED: i32 = -32005;
pub const INVALID_AGENT_RESPONSE: i32 = -32006;
pub const EXTENDED_AGENT_CARD_NOT_CONFIGURED: i32 = -32007;
pub const EXTENSION_SUPPORT_REQUIRED: i32 = -32008;
pub const VERSION_NOT_SUPPORTED: i32 = -32009;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;
pub const PARSE_ERROR: i32 = -32700;

pub const fn http_status_for(code: i32) -> StatusCode {
    match code {
        TASK_NOT_FOUND | METHOD_NOT_FOUND => StatusCode::NOT_FOUND,
        TASK_NOT_CANCELABLE => StatusCode::CONFLICT,
        CONTENT_TYPE_NOT_SUPPORTED => StatusCode::UNSUPPORTED_MEDIA_TYPE,
        INVALID_AGENT_RESPONSE => StatusCode::BAD_GATEWAY,
        PUSH_NOTIFICATION_NOT_SUPPORTED
        | UNSUPPORTED_OPERATION
        | EXTENDED_AGENT_CARD_NOT_CONFIGURED
        | EXTENSION_SUPPORT_REQUIRED
        | VERSION_NOT_SUPPORTED
        | INVALID_REQUEST
        | INVALID_PARAMS
        | PARSE_ERROR => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
