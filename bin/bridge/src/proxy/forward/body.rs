//! Buffers the caller's body before it goes upstream.
//!
//! Covers the size cap, the `OpenCode` session stamp, and the conversation-id
//! derivation that reads the buffered bytes. `OpenCode` speaks the `OpenAI`
//! chat-completions wire and cannot set
//! `metadata.user_id` itself, so its plugin sends the session UUID in
//! `x-opencode-session` and the proxy moves it into the body the gateway keys
//! contexts on. Every other request passes through byte-identical.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::body::Incoming;
use systemprompt_identifiers::{ClientSessionId, GatewayConversationId};

use super::BUFFERED_BODY_LIMIT;
use super::error::{ForwardError, ForwardResult};
use crate::feedback::sessions::OPENCODE_SESSION_HEADER;
use crate::proxy::session::{self, SessionContext};

// Why: the OpenAI-compatible path OpenCode calls; the session stamp applies
// only to bodies bound for it.
pub const CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";

pub(super) async fn prepare_upstream_body(
    body: Incoming,
    session_context: &SessionContext,
    request_headers: &http::HeaderMap,
    request_path: &str,
) -> ForwardResult<(Bytes, Option<GatewayConversationId>)> {
    let buffered = stamp_opencode_session(collect_body(body).await?, request_headers, request_path);
    let id = session::derive_gateway_conversation_id(&buffered)
        .map(|hash| session_context.context_for_prefix(hash));
    if let Some(ref c) = id {
        tracing::Span::current().record("gateway_conversation_id", tracing::field::display(c));
    }
    Ok((buffered, id))
}

async fn collect_body(body: Incoming) -> ForwardResult<Bytes> {
    match http_body_util::Limited::new(body, BUFFERED_BODY_LIMIT)
        .collect()
        .await
    {
        Ok(collected) => Ok(collected.to_bytes()),
        Err(e) if e.is::<http_body_util::LengthLimitError>() => Err(ForwardError::BodyTooLarge),
        Err(e) => Err(ForwardError::ReadBody(e)),
    }
}

pub fn stamp_opencode_session(
    buffered: Bytes,
    request_headers: &http::HeaderMap,
    request_path: &str,
) -> Bytes {
    if !request_path.ends_with(CHAT_COMPLETIONS_PATH) {
        return buffered;
    }
    let Some(session) = request_headers
        .get(OPENCODE_SESSION_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|raw| ClientSessionId::try_new(raw).ok())
    else {
        return buffered;
    };
    let Ok(mut body) = serde_json::from_slice::<serde_json::Value>(&buffered) else {
        return buffered;
    };
    let Some(object) = body.as_object_mut() else {
        return buffered;
    };
    let metadata = object
        .entry("metadata")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let Some(metadata) = metadata.as_object_mut() else {
        return buffered;
    };
    if metadata.contains_key("user_id") {
        return buffered;
    }
    let user_id = serde_json::json!({ "session_id": session.as_str() }).to_string();
    metadata.insert("user_id".to_owned(), serde_json::Value::String(user_id));
    serde_json::to_vec(&body).map_or(buffered, Bytes::from)
}
