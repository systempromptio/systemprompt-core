//! Marketplaces owned by someone else that a marketplace here may depend on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::errors::ConfigValidationError;

/// Where Claude Code fetches a marketplace this instance does not serve.
///
/// Serialises to the `source` object Claude Code accepts in
/// `extraKnownMarketplaces`, so the bridge writes it verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "lowercase", deny_unknown_fields)]
pub enum ExternalMarketplaceSource {
    Github { repo: String },
    Git { url: String },
}

/// A marketplace owned by someone else that plugins here may depend on.
///
/// The gateway never fetches it; the bridge registers it with Claude Code,
/// which clones and installs the dependency plugins itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalMarketplace {
    pub name: String,
    pub source: ExternalMarketplaceSource,
}

impl ExternalMarketplace {
    pub(super) fn validate(&self, key: &str) -> Result<(), ConfigValidationError> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(ConfigValidationError::required(format!(
                "Marketplace '{key}': external_marketplaces entries must be named"
            )));
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(ConfigValidationError::invalid_field(format!(
                "Marketplace '{key}': external marketplace name '{name}' may only contain \
                 letters, digits, '-', '_' and '.'"
            )));
        }
        match &self.source {
            ExternalMarketplaceSource::Github { repo } => {
                let mut parts = repo.split('/');
                let valid = matches!(
                    (parts.next(), parts.next(), parts.next()),
                    (Some(owner), Some(name), None)
                        if !owner.is_empty()
                            && !name.is_empty()
                            && owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
                );
                if !valid {
                    return Err(ConfigValidationError::invalid_field(format!(
                        "Marketplace '{key}': external marketplace '{name}' github repo '{repo}' \
                         must be 'owner/repository'"
                    )));
                }
            },
            ExternalMarketplaceSource::Git { url } => {
                let parsed = crate::net::validate_outbound_url(url).map_err(|e| {
                    ConfigValidationError::invalid_field(format!(
                        "Marketplace '{key}': external marketplace '{name}' git url '{url}' is \
                         not a usable public URL: {e}"
                    ))
                })?;
                if parsed.scheme() != "https" {
                    return Err(ConfigValidationError::invalid_field(format!(
                        "Marketplace '{key}': external marketplace '{name}' git url must use https"
                    )));
                }
            },
        }
        Ok(())
    }
}
