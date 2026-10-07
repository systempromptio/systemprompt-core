//! Scope attribution of an AI request.
//!
//! The value the request is charged to in each subject dimension a tenant
//! registers (`project`, `cost_centre`, ...), which signal decided each value,
//! and the API key that authenticated it.
//!
//! Core owns no organisation model. The entry layer resolves one
//! [`AttributionEntry`] per registered dimension (the
//! `x-systemprompt-scope-<dimension>` header, then the API key's bound value,
//! then the dimension provider's first value) and the gateway carries the set
//! unchanged onto the audit row, the quota windows and the OTLP export.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ApiKeyId, ScopeDimension};

/// Which signal decided an attributed value. Persisted as
/// `ai_request_attributions.source`, a CHECK-constrained column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributionSource {
    Header,
    ApiKey,
    Default,
}

impl AttributionSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::ApiKey => "api_key",
            Self::Default => "default",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "header" => Some(Self::Header),
            "api_key" => Some(Self::ApiKey),
            "default" => Some(Self::Default),
            _ => None,
        }
    }
}

impl std::fmt::Display for AttributionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The value one request is attributed to in one dimension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributionEntry {
    pub dimension: ScopeDimension,
    pub value: String,
    pub source: AttributionSource,
}

/// The resolved scope attribution of one request.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RequestAttribution {
    pub entries: Vec<AttributionEntry>,
    pub api_key_id: Option<ApiKeyId>,
}

impl RequestAttribution {
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn value_for(&self, dimension: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.dimension.as_str() == dimension)
            .map(|entry| entry.value.as_str())
    }
}

/// A value an API key is bound to in one dimension; it attributes the key's
/// requests when they carry no header for that dimension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeBinding {
    pub dimension: ScopeDimension,
    pub value: String,
}
