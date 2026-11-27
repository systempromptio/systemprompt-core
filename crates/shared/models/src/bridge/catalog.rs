//! The model catalog a device reads from `/v1/models`.
//!
//! The profile and whoami bodies are [`crate::bridge::profile`] and
//! [`crate::bridge::gateway`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[must_use]
pub fn base_model_id(id: &str) -> &str {
    match id.find('[') {
        Some(at) if id.ends_with(']') => &id[..at],
        _ => id,
    }
}

/// `GET /v1/models`: the instance-wide catalog, Anthropic shape.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCatalog {
    #[serde(default)]
    pub data: Vec<CatalogModel>,
}

/// One model of the catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogModel {
    pub id: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub max_input_tokens: Option<u64>,
    #[serde(default)]
    pub max_tokens: Option<u64>,
}

impl ModelCatalog {
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&CatalogModel> {
        self.data.iter().find(|model| model.id == id)
    }
}
