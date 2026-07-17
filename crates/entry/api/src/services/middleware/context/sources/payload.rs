//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::Request;
use serde_json::Value;
use systemprompt_models::execution::{ContextExtractionError, ContextIdSource};

#[derive(Debug, Clone, Copy)]
pub struct PayloadSource;

impl PayloadSource {
    pub fn extract_context_source(
        body_bytes: &[u8],
    ) -> Result<ContextIdSource, ContextExtractionError> {
        // JSON: A2A JSON-RPC envelope is an external protocol boundary; the
        // method name drives which typed field is read, so the shape is dynamic.
        let payload: Value = serde_json::from_slice(body_bytes).map_err(|e| {
            ContextExtractionError::InvalidHeaderValue {
                header: "payload".to_owned(),
                reason: format!("Invalid JSON: {e}"),
            }
        })?;

        let method = payload.get("method").and_then(|m| m.as_str()).unwrap_or("");

        if method.starts_with("tasks/") {
            let task_id = payload
                .get("params")
                .and_then(|p| p.get("id"))
                .and_then(|id| id.as_str())
                .map(str::to_owned)
                .ok_or_else(|| ContextExtractionError::InvalidHeaderValue {
                    header: "params.id".to_owned(),
                    reason: "Task ID required for task methods".to_owned(),
                })?;

            return Ok(ContextIdSource::FromTask {
                task_id: systemprompt_identifiers::TaskId::new(task_id),
            });
        }

        payload
            .get("params")
            .and_then(|p| p.get("message"))
            .and_then(|m| m.get("contextId"))
            .and_then(|c| c.as_str())
            .map(|s| ContextIdSource::Direct(s.to_owned()))
            .ok_or(ContextExtractionError::MissingContextId)
    }

    pub async fn read_and_reconstruct(
        request: Request<Body>,
    ) -> Result<(Vec<u8>, Request<Body>), ContextExtractionError> {
        let (parts, body) = request.into_parts();

        let body_bytes = axum::body::to_bytes(body, usize::MAX)
            .await
            .map_err(|e| ContextExtractionError::InvalidHeaderValue {
                header: "body".to_owned(),
                reason: format!("Failed to read body: {e}"),
            })?
            .to_vec();

        let new_body = Body::from(body_bytes.clone());
        let new_request = Request::from_parts(parts, new_body);

        Ok((body_bytes, new_request))
    }
}
