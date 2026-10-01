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
use crate::errors::ConfigValidationError;

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

    fn validate_location(&self) -> Result<(), String> {
        match self {
            Self::Github { repo, .. } => {
                if !is_github_repo(repo) {
                    return Err(format!("github repo '{repo}' must be 'owner/repository'"));
                }
            },
            Self::Url { url, .. } => validate_repository(url)?,
            Self::GitSubdir { url, path, .. } => {
                validate_repository(url)?;
                let relative = path.trim_start_matches("./").trim_end_matches('/');
                crate::managed::validate_path(relative)
                    .map_err(|e| format!("path '{path}' must be a relative path: {e}"))?;
            },
        }
        Ok(())
    }
}

fn validate_repository(url: &str) -> Result<(), String> {
    if is_github_repo(url) {
        return Ok(());
    }
    let parsed = crate::net::validate_outbound_url(url)
        .map_err(|e| format!("url '{url}' is not a usable public URL: {e}"))?;
    if parsed.scheme() != "https" {
        return Err(format!("url '{url}' must use https"));
    }
    Ok(())
}

fn is_commit(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The `skills` override of a pass-through entry: one path or several.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ExternalPluginSkills {
    Path(String),
    Paths(Vec<String>),
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
    fn validate(&self) -> Result<(), String> {
        if !is_external_name(&self.name) {
            return Err("must be named with letters, digits, '-', '_' and '.' only".to_owned());
        }
        self.source.validate_location()?;
        if let Some(reference) = self.source.reference() {
            validate_git_ref(reference)?;
        }
        let sha = self.source.sha();
        if !is_commit(sha) {
            return Err(format!(
                "`sha` '{sha}' must be a full lowercase commit id — a pass-through plugin is \
                 never inspected, so it must be pinned"
            ));
        }
        Ok(())
    }
}

pub(super) fn validate_external_plugins(
    entries: &[ExternalPluginEntry],
    vendored: &[String],
    key: &str,
) -> Result<(), ConfigValidationError> {
    let mut names = BTreeSet::new();
    for entry in entries {
        let name = entry.name.as_str();
        entry.validate().map_err(|e| {
            ConfigValidationError::invalid_field(format!(
                "Marketplace '{key}': external plugin '{name}' {e}"
            ))
        })?;
        if vendored.iter().any(|plugin| plugin == name) {
            return Err(ConfigValidationError::invalid_field(format!(
                "Marketplace '{key}': external plugin '{name}' has the name of a plugin this \
                 marketplace vendors"
            )));
        }
        if !names.insert(name) {
            return Err(ConfigValidationError::invalid_field(format!(
                "Marketplace '{key}': external plugin '{name}' is declared twice"
            )));
        }
    }
    Ok(())
}
