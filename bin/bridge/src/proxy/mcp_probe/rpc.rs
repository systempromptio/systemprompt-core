//! MCP JSON-RPC exchanges used by the probe: initialize, tools/list, parsing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{Value, json};

use super::{MCP_PROTOCOL_VERSION, SESSION_HEADER};

pub(super) async fn list_tools(
    client: &reqwest::Client,
    url: &str,
    bearer: &str,
    session: Option<&str>,
) -> Vec<String> {
    let initialized = with_session(
        client
            .post(url)
            .header(AUTHORIZATION, bearer)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/event-stream"),
        session,
    );
    _ = initialized
        .json(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
        .send()
        .await;

    let req = with_session(
        client
            .post(url)
            .header(AUTHORIZATION, bearer)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json, text/event-stream"),
        session,
    );
    let Ok(resp) = req
        .json(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }))
        .send()
        .await
    else {
        return Vec::new();
    };
    if !resp.status().is_success() {
        return Vec::new();
    }
    let content_type = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let body = resp.text().await.unwrap_or_default();
    parse_tool_names(&content_type, &body)
}

fn with_session(
    builder: reqwest::RequestBuilder,
    session: Option<&str>,
) -> reqwest::RequestBuilder {
    match session {
        Some(s) => builder.header(SESSION_HEADER, s),
        None => builder,
    }
}

pub(super) fn initialize_body() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "systemprompt-bridge-probe", "version": crate::brand::brand().version },
        },
    })
}

fn parse_tool_names(content_type: &str, body: &str) -> Vec<String> {
    let Some(value) = parse_jsonrpc(content_type, body) else {
        return Vec::new();
    };
    value
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_jsonrpc(content_type: &str, body: &str) -> Option<Value> {
    if content_type.contains("text/event-stream") {
        let mut data = String::new();
        for line in body.lines() {
            if let Some(rest) = line.strip_prefix("data:") {
                data.push_str(rest.trim_start());
            }
        }
        serde_json::from_str(&data).ok()
    } else {
        serde_json::from_str(body).ok()
    }
}
