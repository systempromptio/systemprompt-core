//! A2A security schemes read and written in their JSON object form.
//!
//! serde_yaml 0.9 reads an externally tagged enum only as a `!tag`, while the
//! A2A 1.0 security scheme is a single-key object; routing through JSON keeps
//! the spec's `apiKeySecurityScheme: {...}` form in agent YAML.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use systemprompt_models::a2a::SecurityScheme;

#[derive(Debug, thiserror::Error)]
#[error("securitySchemes.{name}: {source}")]
struct InvalidSecuritySchemeError {
    name: String,
    #[source]
    source: serde_json::Error,
}

#[expect(
    clippy::ref_option,
    reason = "serde `with` fixes the serializer signature to &Option<T>"
)]
pub(super) fn serialize<S: Serializer>(
    schemes: &Option<HashMap<String, SecurityScheme>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    // JSON: the spec's single-key object form of each scheme.
    let shaped: Option<HashMap<&String, serde_json::Value>> = schemes
        .as_ref()
        .map(|map| {
            map.iter()
                .map(|(name, scheme)| serde_json::to_value(scheme).map(|v| (name, v)))
                .collect::<Result<_, _>>()
        })
        .transpose()
        .map_err(S::Error::custom)?;
    shaped.serialize(serializer)
}

pub(super) fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<HashMap<String, SecurityScheme>>, D::Error> {
    // JSON: each scheme is read as its spec object, then typed.
    let raw = Option::<HashMap<String, serde_json::Value>>::deserialize(deserializer)?;
    raw.map(|map| {
        map.into_iter()
            .map(|(name, value)| match serde_json::from_value(value) {
                Ok(scheme) => Ok((name, scheme)),
                Err(source) => Err(D::Error::custom(InvalidSecuritySchemeError {
                    name,
                    source,
                })),
            })
            .collect()
    })
    .transpose()
}
