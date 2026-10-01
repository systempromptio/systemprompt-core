//! Marketplaces owned by someone else that a marketplace here may depend on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::marketplace::MarketplaceConfig;
use crate::errors::ConfigValidationError;

/// Where Claude Code fetches a marketplace this instance does not serve.
///
/// Serialises to the `source` object Claude Code accepts in
/// `extraKnownMarketplaces`, so the bridge writes it verbatim. `ref` pins a
/// branch or tag; a marketplace source takes no `sha`, so one is refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "lowercase", deny_unknown_fields)]
pub enum ExternalMarketplaceSource {
    Github {
        repo: String,
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
    },
    Git {
        url: String,
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
    },
}

impl ExternalMarketplaceSource {
    #[must_use]
    pub fn reference(&self) -> Option<&str> {
        match self {
            Self::Github { reference, .. } | Self::Git { reference, .. } => reference.as_deref(),
        }
    }
}

const MAX_GIT_REF_CHARS: usize = 128;

pub(super) fn validate_git_ref(reference: &str) -> Result<(), String> {
    if reference.is_empty() {
        return Err("`ref` must not be empty".to_owned());
    }
    if reference.chars().count() > MAX_GIT_REF_CHARS {
        return Err(format!(
            "`ref` must be at most {MAX_GIT_REF_CHARS} characters"
        ));
    }
    if reference
        .chars()
        .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(format!("`ref` {reference:?} must not contain whitespace"));
    }
    if reference.contains("..") {
        return Err(format!("`ref` {reference:?} must not contain '..'"));
    }
    if reference.starts_with('-') {
        return Err(format!("`ref` {reference:?} must not start with '-'"));
    }
    Ok(())
}

pub(super) fn is_github_repo(repo: &str) -> bool {
    let mut parts = repo.split('/');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(owner), Some(name), None)
            if !owner.is_empty()
                && !name.is_empty()
                && owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    )
}

pub(super) fn is_external_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
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
        if !is_external_name(name) {
            return Err(ConfigValidationError::invalid_field(format!(
                "Marketplace '{key}': external marketplace name '{name}' may only contain \
                 letters, digits, '-', '_' and '.'"
            )));
        }
        match &self.source {
            ExternalMarketplaceSource::Github { repo, .. } => {
                if !is_github_repo(repo) {
                    return Err(ConfigValidationError::invalid_field(format!(
                        "Marketplace '{key}': external marketplace '{name}' github repo '{repo}' \
                         must be 'owner/repository'"
                    )));
                }
            },
            ExternalMarketplaceSource::Git { url, .. } => {
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
        if let Some(reference) = self.source.reference() {
            validate_git_ref(reference).map_err(|e| {
                ConfigValidationError::invalid_field(format!(
                    "Marketplace '{key}': external marketplace '{name}' {e}"
                ))
            })?;
        }
        Ok(())
    }
}

impl MarketplaceConfig {
    #[must_use]
    pub fn external_marketplace(&self, name: &str) -> Option<&ExternalMarketplace> {
        self.external_marketplaces
            .iter()
            .find(|m| m.name.trim() == name)
    }
}
