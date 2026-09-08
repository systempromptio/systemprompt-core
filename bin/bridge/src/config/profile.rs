//! Bridge profile parsing, including the native policy public key section.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;

use systemprompt_identifiers::ValidatedUrl;

use super::{Config, default_gateway};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClaudeConfig {
    #[serde(default)]
    pub inference_gateway_base_url: Option<ValidatedUrl>,
    #[serde(default)]
    pub auth_scheme: Option<String>,
    #[serde(default)]
    pub models: Option<Vec<String>>,
    #[serde(default)]
    pub organization_uuid: Option<String>,
}

#[must_use]
pub fn gateway_url_or_default(cfg: &Config) -> ValidatedUrl {
    let url = cfg.gateway_url.clone().unwrap_or_else(default_gateway);
    tracing::debug!(gateway = %url, "gateway resolved");
    url
}
