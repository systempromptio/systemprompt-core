//! Shaping a neutral request into the HTTP request its protocol expects.
//!
//! [`prepare`] appends the protocol path to the endpoint's base URL, adds the
//! protocol headers the endpoint does not set itself (`content-type`, `accept`,
//! `anthropic-version`), then the endpoint's own headers, and the JSON body
//! from [`request_body`] (endpoint metadata merged in).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use serde_json::{Map, Value};
use url::Url;

use crate::endpoint::Endpoint;
use crate::error::LlmError;
use crate::model::{Protocol, Request};
use crate::transport::HttpRequest;
use crate::{chat, messages};

// JSON: the provider request body, with the endpoint's caller-owned metadata
// merged in.
pub fn request_body(request: &Request, endpoint_metadata: Option<&Value>) -> Value {
    let mut body = match request.model.protocol {
        Protocol::Messages => messages::body(request),
        Protocol::ChatCompletions => chat::body(request),
    };
    if let (Value::Object(target), Some(Value::Object(extra))) = (&mut body, endpoint_metadata) {
        merge(target, extra);
    }
    body
}

pub fn prepare(endpoint: &Endpoint, request: &Request) -> Result<HttpRequest, LlmError> {
    let protocol = request.model.protocol;
    let body = request_body(request, endpoint.metadata.as_ref());
    let body = serde_json::to_vec(&body).map_err(LlmError::Encode)?;
    let url = protocol_url(&endpoint.base_url, protocol)?;
    let headers = headers(endpoint, protocol);
    Ok(HttpRequest {
        url,
        headers,
        body: Bytes::from(body),
    })
}

fn protocol_url(base: &Url, protocol: Protocol) -> Result<Url, LlmError> {
    let mut url = base.clone();
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|()| LlmError::InvalidEndpoint)?;
        segments.pop_if_empty();
        match protocol {
            Protocol::Messages => segments.push("messages"),
            Protocol::ChatCompletions => segments.push("chat").push("completions"),
        };
    }
    Ok(url)
}

fn headers(endpoint: &Endpoint, protocol: Protocol) -> Vec<(String, String)> {
    let has = |name: &str| {
        endpoint
            .headers
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case(name))
    };
    let mut headers = Vec::with_capacity(endpoint.headers.len() + 3);
    if !has("content-type") {
        headers.push(("content-type".to_owned(), "application/json".to_owned()));
    }
    if !has("accept") {
        headers.push(("accept".to_owned(), "text/event-stream".to_owned()));
    }
    if protocol == Protocol::Messages && !has("anthropic-version") {
        headers.push((
            "anthropic-version".to_owned(),
            messages::API_VERSION.to_owned(),
        ));
    }
    headers.extend(endpoint.headers.iter().cloned());
    headers
}

// JSON: recursive merge of two provider-body objects; objects merge, anything
// else replaces.
fn merge(target: &mut Map<String, Value>, extra: &Map<String, Value>) {
    for (key, value) in extra {
        match (target.get_mut(key), value) {
            (Some(Value::Object(existing)), Value::Object(incoming)) => merge(existing, incoming),
            _ => {
                target.insert(key.clone(), value.clone());
            },
        }
    }
}
