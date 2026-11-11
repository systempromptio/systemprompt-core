//! Request-context extraction from A2A JSON-RPC bodies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::Request;
use bytes::Bytes;
use serde::Deserialize;
use serde_json::Value;
use systemprompt_models::execution::{ContextExtractionError, ContextIdSource};

use crate::services::middleware::body::{BodyReadError, read_request_bounded};

// JSON: the A2A JSON-RPC envelope is an external protocol boundary. Only the
// routing fields are typed; every other field (message parts, file bytes) is
// skipped by serde without being allocated, so parsing costs no second copy
// of the body.
#[derive(Debug, Deserialize)]
struct Envelope {
    // JSON: JSON-RPC lets a client send any JSON type here; a non-string is refused downstream.
    #[serde(default)]
    method: Option<Value>,
    #[serde(default)]
    params: Option<Params>,
}

#[derive(Debug, Deserialize)]
struct Params {
    // JSON: a JSON-RPC id may be a string, number or null.
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    message: Option<Message>,
}

#[derive(Debug, Deserialize)]
struct Message {
    // JSON: a client-supplied contextId is validated after parsing, whatever its JSON type.
    #[serde(default, rename = "contextId")]
    context_id: Option<Value>,
}

#[derive(Debug, Clone, Copy)]
pub struct PayloadSource;

impl PayloadSource {
    pub fn extract_context_source(
        body_bytes: &[u8],
    ) -> Result<ContextIdSource, ContextExtractionError> {
        let envelope: Envelope = serde_json::from_slice(body_bytes).map_err(|error| {
            tracing::debug!(%error, "request payload is not valid JSON");
            ContextExtractionError::InvalidHeaderValue {
                header: "payload".to_owned(),
                reason: "body is not valid JSON".to_owned(),
            }
        })?;

        let method = envelope
            .method
            .as_ref()
            .and_then(Value::as_str)
            .unwrap_or("");
        let params = envelope.params;

        if method.starts_with("tasks/") {
            let task_id = params
                .as_ref()
                .and_then(|p| p.id.as_ref())
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| ContextExtractionError::InvalidHeaderValue {
                    header: "params.id".to_owned(),
                    reason: "Task ID required for task methods".to_owned(),
                })?;

            return Ok(ContextIdSource::FromTask {
                task_id: systemprompt_identifiers::TaskId::new(task_id),
            });
        }

        params
            .and_then(|p| p.message)
            .and_then(|m| m.context_id)
            .as_ref()
            .and_then(Value::as_str)
            .map(|s| ContextIdSource::Direct(s.to_owned()))
            .ok_or(ContextExtractionError::MissingContextId)
    }

    pub async fn read_and_reconstruct(
        request: Request<Body>,
        body_limit: usize,
    ) -> Result<(Bytes, Request<Body>), ContextExtractionError> {
        let (parts, body_bytes) =
            read_request_bounded(request, body_limit)
                .await
                .map_err(|error| match error {
                    BodyReadError::TooLarge { limit } => {
                        ContextExtractionError::PayloadTooLarge { limit }
                    },
                    BodyReadError::Unreadable(source) => {
                        tracing::debug!(error = %source, "request body could not be read");
                        ContextExtractionError::InvalidHeaderValue {
                            header: "body".to_owned(),
                            reason: "request body could not be read".to_owned(),
                        }
                    },
                })?;

        let new_request = Request::from_parts(parts, Body::from(body_bytes.clone()));

        Ok((body_bytes, new_request))
    }
}
