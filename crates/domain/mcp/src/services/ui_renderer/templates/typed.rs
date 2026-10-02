//! Typed extraction of an artifact payload from an A2A [`Artifact`].
//!
//! Renderers for artifacts with a fixed schema (card, message, media) decode
//! the data part straight into its model type rather than poking at loose
//! JSON. The CLI envelope's `artifact_type` tag rides alongside the payload
//! fields, so the flattened form deserializes without a separate unwrap step.
//!
//! Renderers for loosely-shaped hints (form fields, list items, table
//! columns) decode through [`lenient`] and [`lenient_vec`]: a field of the
//! wrong JSON type reads as absent and a malformed list entry is skipped,
//! rather than failing the whole artifact.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{McpDomainError, McpDomainResult};
use serde::de::{Deserialize, DeserializeOwned, Deserializer};
use serde_json::Value as JsonValue;
use systemprompt_models::a2a::{Artifact, Part};

pub(super) fn artifact_payload<T: DeserializeOwned>(artifact: &Artifact) -> McpDomainResult<T> {
    let data = artifact
        .parts
        .iter()
        .find_map(Part::as_data)
        .ok_or_else(|| McpDomainError::ArtifactHasNoData)?;

    serde_json::from_value(data).map_err(|e| {
        McpDomainError::operation("Artifact payload does not match its declared type", e)
    })
}

pub(super) fn lenient<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = JsonValue::deserialize(deserializer)?;
    Ok(T::deserialize(value).ok())
}

pub(super) fn lenient_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = JsonValue::deserialize(deserializer)?;
    Ok(value.as_array().map_or_else(Vec::new, |items| {
        items
            .iter()
            .filter(|item| !item.is_array())
            .filter_map(|item| T::deserialize(item).ok())
            .collect()
    }))
}
