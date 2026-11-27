//! JSON-RPC 2.0 framing for the messages a client sends and receives.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Map, Value};

use crate::error::McpError;

pub(crate) const METHOD_NOT_FOUND: i64 = -32601;
const INTERNAL_ERROR: i64 = -32603;

// JSON: a JSON-RPC 2.0 request frame; `params` is the called method's own
// payload.
pub(crate) fn request(id: u64, method: &str, params: Option<Value>) -> Value {
    let mut message = envelope();
    message.insert("id".into(), Value::from(id));
    message.insert("method".into(), Value::from(method));
    if let Some(params) = params {
        message.insert("params".into(), params);
    }
    Value::Object(message)
}

// JSON: a JSON-RPC 2.0 notification frame; `params` is the method's own
// payload.
pub(crate) fn notification(method: &str, params: Option<Value>) -> Value {
    let mut message = envelope();
    message.insert("method".into(), Value::from(method));
    if let Some(params) = params {
        message.insert("params".into(), params);
    }
    Value::Object(message)
}

// JSON: a JSON-RPC 2.0 success frame echoing the server's request id verbatim.
pub(crate) fn result(id: Value, result: Value) -> Value {
    let mut message = envelope();
    message.insert("id".into(), id);
    message.insert("result".into(), result);
    Value::Object(message)
}

// JSON: a JSON-RPC 2.0 error frame echoing the server's request id verbatim.
pub(crate) fn error(id: Value, code: i64, text: &str) -> Value {
    let mut body = Map::new();
    body.insert("code".into(), Value::from(code));
    body.insert("message".into(), Value::from(text));
    let mut message = envelope();
    message.insert("id".into(), id);
    message.insert("error".into(), Value::Object(body));
    Value::Object(message)
}

// JSON: the members every JSON-RPC 2.0 frame starts from.
fn envelope() -> Map<String, Value> {
    let mut message = Map::new();
    message.insert("jsonrpc".into(), Value::from("2.0"));
    message
}

// JSON: reads the `method` member of a JSON-RPC 2.0 frame.
pub(crate) fn method(message: &Value) -> Option<&str> {
    message.get("method").and_then(Value::as_str)
}

// JSON: a JSON-RPC 2.0 frame is a request when it has a method and a non-null
// id.
pub(crate) fn is_request(message: &Value) -> bool {
    method(message).is_some() && message.get("id").is_some_and(|id| !id.is_null())
}

#[derive(Debug)]
pub(crate) enum Incoming {
    Response {
        id: u64,
        // JSON: a response's `result` member is the called method's own payload.
        outcome: Result<Value, McpError>,
    },
    Request {
        // JSON: a server request's id is any JSON value and is echoed back verbatim.
        id: Value,
        method: String,
    },
    Notification {
        method: String,
        // JSON: a notification's `params` member is the method's own payload.
        params: Value,
    },
    Invalid,
}

// JSON: one decoded JSON-RPC 2.0 frame from the server.
pub(crate) fn classify(message: Value) -> Incoming {
    let Value::Object(mut object) = message else {
        return Incoming::Invalid;
    };
    let id = object.remove("id").filter(|id| !id.is_null());
    let method = object
        .get("method")
        .and_then(Value::as_str)
        .map(str::to_owned);
    match (method, id) {
        (Some(method), Some(id)) => Incoming::Request { id, method },
        (Some(method), None) => Incoming::Notification {
            method,
            params: object.remove("params").unwrap_or(Value::Null),
        },
        (None, Some(id)) => response_id(&id).map_or(Incoming::Invalid, |id| Incoming::Response {
            id,
            outcome: outcome(&mut object),
        }),
        (None, None) => Incoming::Invalid,
    }
}

// JSON: the members of a JSON-RPC 2.0 response frame.
fn outcome(object: &mut Map<String, Value>) -> Result<Value, McpError> {
    object.remove("error").map_or_else(
        || Ok(object.remove("result").unwrap_or(Value::Null)),
        |error| {
            Err(McpError::Rpc {
                code: error
                    .get("code")
                    .and_then(Value::as_i64)
                    .unwrap_or(INTERNAL_ERROR),
                message: error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                data: error.get("data").cloned().map(Box::new),
            })
        },
    )
}

// JSON: a response id, which this client sends as a number and some servers
// echo as a string.
fn response_id(id: &Value) -> Option<u64> {
    match id {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}
