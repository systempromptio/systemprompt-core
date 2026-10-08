//! Plugins a marketplace lists but never vendors.
//!
//! A pass-through entry is kept exactly as authored and handed to Claude Code,
//! which fetches the pinned commit itself — the gateway never clones it. That
//! is how a plugin whose repository is not a marketplace (no
//! `marketplace.json`, often no `plugin.json`) becomes a dependency target:
//! it is listed in the marketplace that depends on it. Because nothing here
//! inspects the content, every entry must pin a full commit `sha`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::marketplace_external::{is_external_name, is_github_repo, validate_git_ref};
use super::marketplace_external_error::ExternalEntryError;
use systemprompt_models::bridge::manifest::ExternalPluginSkills;
use systemprompt_models::errors::ServicesValidationError;

/// Where Claude Code fetches a pass-through plugin.
///
/// Serialises to the plugin `source` object of Claude Code's
/// `marketplace.json`; `ref` is optional, `sha` is required.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ExternalPluginSource {
    Github {
        repo: String,
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
        sha: String,
    },
    Url {
        url: String,
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
        sha: String,
    },
    GitSubdir {
        url: String,
        path: String,
        #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
        reference: Option<String>,
        sha: String,
    },
}

impl ExternalPluginSource {
    #[must_use]
    pub fn reference(&self) -> Option<&str> {
        match self {
            Self::Github { reference, .. }
            | Self::Url { reference, .. }
            | Self::GitSubdir { reference, .. } => reference.as_deref(),
        }
    }

    #[must_use]
    pub fn sha(&self) -> &str {
        match self {
            Self::Github { sha, .. } | Self::Url { sha, .. } | Self::GitSubdir { sha, .. } => sha,
        }
    }

    fn validate_location(&self) -> Result<(), ExternalEntryError> {
        match self {
            Self::Github { repo, .. } => {
                if !is_github_repo(repo) {
                    return Err(ExternalEntryError::GithubRepo(repo.clone()));
                }
            },
            Self::Url { url, .. } => validate_repository(url)?,
            Self::GitSubdir { url, path, .. } => {
                validate_repository(url)?;
                let relative = path.trim_start_matches("./").trim_end_matches('/');
                systemprompt_models::managed::validate_path(relative).map_err(|source| {
                    ExternalEntryError::Path {
                        path: path.clone(),
                        source,
                    }
                })?;
            },
        }
        Ok(())
    }
}

fn validate_repository(url: &str) -> Result<(), ExternalEntryError> {
    if is_github_repo(url) {
        return Ok(());
    }
    let parsed = systemprompt_models::net::validate_outbound_url(url).map_err(|source| {
        ExternalEntryError::UnusableUrl {
            url: url.to_owned(),
            source,
        }
    })?;
    if parsed.scheme() != "https" {
        return Err(ExternalEntryError::NotHttps(url.to_owned()));
    }
    Ok(())
}

fn is_commit(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A plugin entry passed through to Claude Code's `marketplace.json`.
///
/// With `strict: false` the entry itself is the plugin manifest, so an
/// upstream without a `plugin.json` still installs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalPluginEntry {
    pub name: String,
    pub source: ExternalPluginSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<ExternalPluginSkills>,
}

impl ExternalPluginEntry {
    fn validate(&self) -> Result<(), ExternalEntryError> {
        if !is_external_name(&self.name) {
            return Err(ExternalEntryError::InvalidName);
        }
        self.source.validate_location()?;
        if let Some(reference) = self.source.reference() {
            validate_git_ref(reference)?;
        }
        let sha = self.source.sha();
        if !is_commit(sha) {
            return Err(ExternalEntryError::UnpinnedSha(sha.to_owned()));
        }
        Ok(())
    }
}

pub(super) fn validate_external_plugins(
    entries: &[ExternalPluginEntry],
    vendored: &[String],
    key: &str,
) -> Result<(), ServicesValidationError> {
    let mut names = BTreeSet::new();
    for entry in entries {
        let name = entry.name.as_str();
        entry.validate().map_err(|e| {
            ServicesValidationError::invalid_field_cause(
                format!("Marketplace '{key}': external plugin '{name}'"),
                e,
            )
        })?;
        if vendored.iter().any(|plugin| plugin == name) {
            return Err(ServicesValidationError::invalid_field(format!(
                "Marketplace '{key}': external plugin '{name}' has the name of a plugin this \
                 marketplace vendors"
            )));
        }
        if !names.insert(name) {
            return Err(ServicesValidationError::invalid_field(format!(
                "Marketplace '{key}': external plugin '{name}' is declared twice"
            )));
        }
    }
    Ok(())
}
