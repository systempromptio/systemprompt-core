use axum::body::Body;
use axum::extract::Request;
use serde_json::Value;
use systemprompt_models::execution::ContextExtractionError;

/// Result of context extraction from A2A payload.
/// Per A2A spec, message methods include contextId directly,
/// while task methods only have task ID (context resolved from storage).
#[derive(Debug, Clone)]
pub enum ContextIdSource {
    /// contextId found directly in payload (message/send, message/stream)
    Direct(String),
    /// Task-based method - context should be resolved from task storage
    FromTask { task_id: String },
}

#[derive(Debug, Clone, Copy)]
pub struct PayloadSource;

impl PayloadSource {
    /// Extract context information from A2A JSON-RPC payload.
    /// Returns either a direct contextId or task_id for resolution.
    pub fn extract_context_source(body_bytes: &[u8]) -> Result<ContextIdSource, ContextExtractionError> {
        let payload: Value = serde_json::from_slice(body_bytes).map_err(|e| {
            ContextExtractionError::InvalidHeaderValue {
                header: "payload".to_string(),
                reason: format!("Invalid JSON: {e}"),
            }
        })?;

        let method = payload
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("");

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

    /// Legacy method for backwards compatibility - extracts contextId directly.
    /// Prefer `extract_context_source` for A2A spec compliance.
    pub fn extract_context_id(body_bytes: &[u8]) -> Result<String, ContextExtractionError> {
        match Self::extract_context_source(body_bytes)? {
            ContextIdSource::Direct(id) => Ok(id),
            ContextIdSource::FromTask { task_id } => {
                // For task-based methods, we need context resolution from storage
                // This is handled by the TaskContextResolver
                Err(ContextExtractionError::InvalidHeaderValue {
                    header: "contextId".to_string(),
                    reason: format!(
                        "Task-based method requires context resolution from task storage (task_id: {})",
                        task_id
                    ),
                })
            }
        }
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
