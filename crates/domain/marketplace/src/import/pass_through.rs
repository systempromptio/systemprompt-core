//! `marketplace.json` entries the importer keeps as authored.
//!
//! A `"mode": "pass_through"` entry is never fetched. It becomes an
//! [`ExternalPluginEntry`] on the marketplace config, which the manifest
//! carries and the bridge appends to the catalog it writes, so Claude Code
//! fetches the pinned commit itself. Only the keys that entry type models are
//! accepted, and its source must pin a `sha`: content nobody here inspects is
//! at least fixed to one commit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_manifest::services::{ExternalPluginEntry, ExternalPluginSource};
use systemprompt_models::bridge::manifest::ExternalPluginSkills;

use crate::error::MarketplaceError;

use super::anthropic::{MarketplacePluginEntry, PluginEntryMode};

pub(super) fn split<'a>(
    entries: &'a [MarketplacePluginEntry],
    manifest_path: &Path,
) -> Result<(Vec<&'a MarketplacePluginEntry>, Vec<ExternalPluginEntry>), MarketplaceError> {
    let (passed, vendored): (Vec<_>, Vec<_>) = entries
        .iter()
        .partition(|entry| entry.mode == PluginEntryMode::PassThrough);
    let external = passed
        .into_iter()
        .map(|entry| external_plugin(entry, manifest_path))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((vendored, external))
}

fn external_plugin(
    entry: &MarketplacePluginEntry,
    manifest_path: &Path,
) -> Result<ExternalPluginEntry, MarketplaceError> {
    let refuse = |message: String| MarketplaceError::Import {
        path: manifest_path.display().to_string(),
        message: format!("pass-through plugin '{}': {message}", entry.name),
    };
    let unmodelled = unmodelled_keys(entry);
    if !unmodelled.is_empty() {
        return Err(refuse(format!(
            "may only carry name, source, description, version, strict and skills; remove {}",
            unmodelled.join(", ")
        )));
    }
    let source = entry
        .source
        .clone()
        .ok_or_else(|| refuse("needs a github, url or git-subdir `source`".to_owned()))?;
    let malformed = |context: &str, source: serde_json::Error| {
        MarketplaceError::import(
            manifest_path,
            format!("pass-through plugin '{}': {context}", entry.name),
            source,
        )
    };
    let source: ExternalPluginSource = serde_json::from_value(source).map_err(|e| {
        malformed(
            "`source` must be a github, url or git-subdir object pinned by `sha`",
            e,
        )
    })?;
    let skills: Option<ExternalPluginSkills> = entry
        .skills
        .clone()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| malformed("`skills` must be a path or a list of paths", e))?;
    Ok(ExternalPluginEntry {
        name: entry.name.clone(),
        source,
        description: entry.description.clone(),
        version: entry.version.clone(),
        strict: entry.strict,
        skills,
    })
}

fn unmodelled_keys(entry: &MarketplacePluginEntry) -> Vec<&'static str> {
    [
        ("category", entry.category.is_some()),
        ("keywords", !entry.keywords.is_empty()),
        ("author", entry.author.is_some()),
        ("license", entry.license.is_some()),
        ("homepage", entry.homepage.is_some()),
        ("repository", entry.repository.is_some()),
        ("tags", !entry.tags.is_empty()),
    ]
    .into_iter()
    .filter_map(|(key, present)| present.then_some(key))
    .collect()
}
