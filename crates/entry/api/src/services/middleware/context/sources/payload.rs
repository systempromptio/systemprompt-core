use axum::body::Body;
use axum::extract::Request;
use serde_json::Value;
use systemprompt_models::execution::{ContextExtractionError, ContextIdSource};

#[derive(Debug, Clone, Copy)]
pub struct PayloadSource;

impl PayloadSource {
    /// Extract context information from A2A JSON-RPC payload.
    /// Returns either a direct contextId or task_id for resolution.
    pub fn extract_context_source(
        body_bytes: &[u8],
    ) -> Result<ContextIdSource, ContextExtractionError> {
        let payload: Value = serde_json::from_slice(body_bytes).map_err(|e| {
            ContextExtractionError::InvalidHeaderValue {
                header: "payload".to_string(),
                reason: format!("Invalid JSON: {e}"),
            }
        })?;

        let method = payload.get("method").and_then(|m| m.as_str()).unwrap_or("");

        // Per A2A spec Section 7.3: task methods use TaskQueryParams/TaskIdParams
        // which only have 'id' (task UUID), not contextId
        if method.starts_with("tasks/") {
            let task_id = payload
                .get("params")
                .and_then(|p| p.get("id"))
                .and_then(|id| id.as_str())
                .map(ToString::to_string)
                .ok_or_else(|| ContextExtractionError::InvalidHeaderValue {
                    header: "params.id".to_string(),
                    reason: "Task ID required for task methods".to_string(),
                })?;

            return Ok(ContextIdSource::FromTask { task_id });
        }

        // Per A2A spec Section 7.1: message methods use MessageSendParams
        // which has message.contextId
        payload
            .get("params")
            .and_then(|p| p.get("message"))
            .and_then(|m| m.get("contextId"))
            .and_then(|c| c.as_str())
            .map(|s| ContextIdSource::Direct(s.to_string()))
            .ok_or(ContextExtractionError::MissingContextId)
    }

    pub async fn read_and_reconstruct(
        request: Request<Body>,
    ) -> Result<(Vec<u8>, Request<Body>), ContextExtractionError> {
        let (parts, body) = request.into_parts();

        let body_bytes = axum::body::to_bytes(body, usize::MAX)
            .await
            .map_err(|e| ContextExtractionError::InvalidHeaderValue {
                header: "body".to_string(),
                reason: format!("Failed to read body: {e}"),
            })?
            .to_vec();

        let new_body = Body::from(body_bytes.clone());
        let new_request = Request::from_parts(parts, new_body);

        Ok((body_bytes, new_request))
    }
}
