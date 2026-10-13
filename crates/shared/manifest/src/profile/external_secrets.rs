//! Profile `secrets.external:` block — a secrets source supplied by an
//! extension.
//!
//! Core knows only the `kind` that selects a registered provider and the
//! optional refresh interval; every other key is handed to that provider
//! untouched, so a new backend needs no change to the profile schema.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// Lowest accepted `refresh_secs`; bounds how often a reload may run.
pub const MIN_EXTERNAL_REFRESH_SECS: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExternalSecretsConfig {
    pub kind: String,

    #[serde(default)]
    pub refresh_secs: Option<u64>,

    // JSON: provider-specific settings; their shape belongs to the extension
    // registered for `kind`, not to core.
    #[serde(flatten)]
    pub config: serde_json::Map<String, serde_json::Value>,
}
