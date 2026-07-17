//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use systemprompt_models::execution::ContextExtractionError;

#[derive(Debug, Clone, Copy)]
pub struct HeaderSource;

impl HeaderSource {
    pub fn extract_required(
        headers: &HeaderMap,
        name: &str,
    ) -> Result<String, ContextExtractionError> {
        headers
            .get(name)
            .ok_or_else(|| ContextExtractionError::MissingHeader(name.to_owned()))?
            .to_str()
            .map(str::to_owned)
            .map_err(|e| ContextExtractionError::InvalidHeaderValue {
                header: name.to_owned(),
                reason: e.to_string(),
            })
    }

    pub fn extract_optional(headers: &HeaderMap, name: &str) -> Option<String> {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    }
}
