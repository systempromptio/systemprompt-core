//! `OpenCode` client settings a marketplace declares for its subscribers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use systemprompt_models::bridge::ids::Sha256Digest;
use systemprompt_models::bridge::opencode::MIN_CLIENT_VERSION;
use systemprompt_models::errors::ServicesValidationError;

/// `OpenCode` settings the bridge applies for a marketplace's subscribers.
///
/// `theme` names `hosts/opencode/themes/<name>.json` in one of the
/// marketplace's plugin bundles and pins its digest; `cli_defaults` are
/// top-level `opencode.json` keys set only where the user has not set them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenCodeMarketplaceConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_client_version: Option<semver::Version>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<OpenCodeThemeConfig>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    // JSON: client defaults are foreign `opencode.json` keys, passed through
    pub cli_defaults: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenCodeThemeConfig {
    pub name: String,
    pub sha256: Sha256Digest,
}

const RESERVED_DEFAULTS: &[&str] = &["$schema", "providers", "policies", "mcp", "model"];

impl OpenCodeMarketplaceConfig {
    pub(super) fn validate(&self, key: &str) -> Result<(), ServicesValidationError> {
        if let Some(version) = &self.min_client_version
            && *version < MIN_CLIENT_VERSION
        {
            return Err(ServicesValidationError::invalid_field(format!(
                "Marketplace '{key}': opencode.min_client_version must be at least \
                 {MIN_CLIENT_VERSION}"
            )));
        }
        if let Some(theme) = &self.theme
            && (theme.name.is_empty()
                || !theme
                    .name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        {
            return Err(ServicesValidationError::invalid_field(format!(
                "Marketplace '{key}': opencode.theme.name must be a plain file stem"
            )));
        }
        if let Some(reserved) = self
            .cli_defaults
            .keys()
            .find(|k| RESERVED_DEFAULTS.contains(&k.as_str()))
        {
            return Err(ServicesValidationError::invalid_field(format!(
                "Marketplace '{key}': opencode.cli_defaults may not set '{reserved}'"
            )));
        }
        Ok(())
    }
}
