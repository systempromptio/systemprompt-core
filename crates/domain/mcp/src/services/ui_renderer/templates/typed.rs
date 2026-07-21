//! Typed extraction of an artifact payload from an A2A [`Artifact`].
//!
//! Renderers for artifacts with a fixed schema (card, message, media) decode
//! the data part straight into its model type rather than poking at loose
//! JSON. The CLI envelope's `artifact_type` tag rides alongside the payload
//! fields, so the flattened form deserializes without a separate unwrap step.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{McpDomainError, McpDomainResult};
use serde::de::DeserializeOwned;
use systemprompt_models::a2a::{Artifact, Part};

pub(super) fn artifact_payload<T: DeserializeOwned>(artifact: &Artifact) -> McpDomainResult<T> {
    let data = artifact
        .parts
        .iter()
        .find_map(Part::as_data)
        .ok_or_else(|| {
            McpDomainError::Internal("Artifact has no data part to render".to_owned())
        })?;

    serde_json::from_value(data).map_err(|e| {
        McpDomainError::Internal(format!(
            "Artifact payload does not match its declared type: {e}"
        ))
    })
}
