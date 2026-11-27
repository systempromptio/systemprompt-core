//! Sending a call and resolving its response head.
//!
//! [`stream`] resolves at the response head: a non-2xx answer becomes
//! `LlmError::Status` or `LlmError::ContextOverflow`; otherwise it hands back
//! the head and a stream that yields events until `Finish`, an error, or
//! cancellation, which yields `LlmError::Aborted` once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::pin::Pin;
use std::time::Duration;

use futures_util::{Stream, StreamExt};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::endpoint::Endpoint;
use crate::error::{LlmError, excerpt};
use crate::event::Event;
use crate::model::Request;
use crate::overflow::is_context_overflow;
use crate::prepare::prepare;
use crate::response::{ModelResponse, ResponseHead};
use crate::transport::{ByteStream, HttpResponse, Transport};
use crate::{driver, retry};

/// The unified events of one model answer; it ends after `Finish` or after the
/// first error.
pub type EventStream = Pin<Box<dyn Stream<Item = Result<Event, LlmError>> + Send>>;

const MAX_ERROR_BODY: usize = 16 * 1024;
const ERROR_BODY_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn stream(
    transport: &dyn Transport,
    endpoint: &Endpoint,
    request: &Request,
    cancel: CancellationToken,
) -> Result<ModelResponse, LlmError> {
    let prepared = prepare(endpoint, request)?;
    let protocol = request.model.protocol;
    tracing::debug!(model = %request.model.id, ?protocol, "sending model request");
    let response = tokio::select! {
        biased;
        () = cancel.cancelled() => return Err(LlmError::Aborted),
        sent = transport.send(prepared) => sent?,
    };
    if !(200..300).contains(&response.status) {
        return Err(status_error(response, &cancel).await);
    }
    tracing::debug!(model = %request.model.id, status = response.status, "model stream opened");
    let HttpResponse {
        status,
        headers,
        body,
    } = response;
    Ok(ModelResponse {
        head: ResponseHead { status, headers },
        events: driver::events(body, protocol, cancel),
    })
}

async fn status_error(response: HttpResponse, cancel: &CancellationToken) -> LlmError {
    let HttpResponse {
        status,
        headers,
        body,
    } = response;
    let retry_after = retry::retry_after(&headers);
    let bytes = tokio::select! {
        biased;
        () = cancel.cancelled() => return LlmError::Aborted,
        outcome = tokio::time::timeout(ERROR_BODY_TIMEOUT, read_error_body(body)) => {
            // Why: discard-ok: an error body that does not arrive in time leaves the excerpt empty;
            // the HTTP status is still reported.
            outcome.unwrap_or_default()
        },
    };
    let text = String::from_utf8_lossy(&bytes);
    let message = error_message(&text);
    if is_context_overflow(Some(status), &text) {
        tracing::info!(status, "model request exceeds the context window");
        return LlmError::ContextOverflow {
            status: Some(status),
            message: excerpt(&message),
        };
    }
    tracing::warn!(status, "model request refused by the gateway");
    LlmError::Status {
        status,
        retry_after,
        body_excerpt: excerpt(&message),
    }
}

async fn read_error_body(mut body: ByteStream) -> Vec<u8> {
    let mut buffer = Vec::new();
    while let Some(Ok(chunk)) = body.next().await {
        buffer.extend_from_slice(&chunk);
        if buffer.len() >= MAX_ERROR_BODY {
            buffer.truncate(MAX_ERROR_BODY);
            break;
        }
    }
    buffer
}

fn error_message(text: &str) -> String {
    let Ok(parsed) = serde_json::from_str::<Value>(text) else {
        return text.to_owned();
    };
    let field = |pointer: &str| {
        parsed
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    field("/error/message")
        .or_else(|| field("/message"))
        .or_else(|| field("/error"))
        .unwrap_or_else(|| text.to_owned())
}
