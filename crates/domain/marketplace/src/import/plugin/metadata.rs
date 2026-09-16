//! Plugin metadata precedence: the plugin manifest wins, the marketplace
//! entry fills what it leaves blank, and a missing category falls back with a
//! recorded warning.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::bridge::plugin_bundle::PluginManifest;
use systemprompt_models::services::plugin::PluginAuthor;

use super::super::anthropic::MarketplacePluginEntry;
use super::super::marketplace::DEFAULT_VERSION;
use super::super::sidecar::PluginSidecar;
use super::super::warning::ImportWarning;
use super::FALLBACK_CATEGORY;

pub(super) fn resolve_category(
    id: &str,
    sidecar: &PluginSidecar,
    entry: &MarketplacePluginEntry,
    warnings: &mut Vec<ImportWarning>,
) -> String {
    sidecar
        .plugin
        .category
        .clone()
        .or_else(|| entry.category.clone())
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(|| {
            warnings.push(ImportWarning::MissingCategory {
                plugin: id.to_owned(),
                applied: FALLBACK_CATEGORY.to_owned(),
            });
            FALLBACK_CATEGORY.to_owned()
        })
}

pub(super) fn description(manifest: &PluginManifest, entry: &MarketplacePluginEntry) -> String {
    if manifest.description.trim().is_empty() {
        entry.description.clone().unwrap_or_default()
    } else {
        manifest.description.clone()
    }
}

pub(super) fn version(manifest: &PluginManifest, entry: &MarketplacePluginEntry) -> String {
    if manifest.version.trim().is_empty() {
        entry
            .version
            .clone()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_VERSION.to_owned())
    } else {
        manifest.version.clone()
    }
}

pub(super) fn author(manifest: &PluginManifest, entry: &MarketplacePluginEntry) -> PluginAuthor {
    manifest.author.as_ref().map_or_else(
        || PluginAuthor {
            name: entry.author_name().unwrap_or_default(),
            email: entry.author_email().unwrap_or_default(),
        },
        |a| PluginAuthor {
            name: a.name.clone(),
            email: a.email.clone(),
        },
    )
}

pub(super) fn keywords(manifest: &PluginManifest, entry: &MarketplacePluginEntry) -> Vec<String> {
    if manifest.keywords.is_empty() {
        let mut out = entry.keywords.clone();
        out.extend(entry.tags.iter().cloned());
        out.sort();
        out.dedup();
        out
    } else {
        manifest.keywords.clone()
    }
}
