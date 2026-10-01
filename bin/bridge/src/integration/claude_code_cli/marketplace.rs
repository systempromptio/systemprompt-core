//! Marketplace discovery and the user's plugin registry.
//!
//! Covers `marketplace.json`, `known_marketplaces.json`,
//! `installed_plugins.json`, and the `settings.json` enablement entries, each
//! keyed by the marketplace being written. Every registry file is updated in
//! place so the user's own marketplaces and plugins survive untouched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::{Value, json};
use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::bridge::manifest::{ManifestExternalMarketplace, ManifestExternalPlugin};

use super::foreign::{self, ForeignRefs};
use super::io_err;
use super::layout::{marketplace_dir, plugin_key};
use super::sidecar::Owned;
use crate::config::paths;
use crate::gateway::manifest::SignedManifest;
use crate::host_sync::ApplyError;
use crate::ids::PluginId;
use crate::integration::json_io::{
    object_entry, read_json_object, read_optional_object, write_json,
};

/// A Claude Code marketplace this emitter mirrors: the gateway marketplace's
/// id and name, the manifest plugins it carries, and the pass-through plugins
/// its catalog lists for Claude Code to fetch itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostMarketplace {
    pub id: MarketplaceId,
    pub name: String,
    pub plugin_ids: Vec<PluginId>,
    pub allow_cross_marketplace_dependencies_on: Vec<String>,
    pub external_marketplaces: Vec<ManifestExternalMarketplace>,
    pub external_plugins: Vec<ManifestExternalPlugin>,
}

impl HostMarketplace {
    // Why: Claude Code caches and records a pass-through plugin under this
    // marketplace's key, so a sync must not sweep it as stale.
    pub(super) fn retained<'a>(&'a self, mirrored: &[&'a PluginId]) -> Vec<&'a str> {
        mirrored
            .iter()
            .map(|id| id.as_str())
            .chain(self.external_plugins.iter().map(|p| p.name.as_str()))
            .collect()
    }
}

/// The plugins mirrored under one marketplace, as written to `settings.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mirrored {
    pub id: MarketplaceId,
    pub plugin_ids: Vec<PluginId>,
}

#[must_use]
pub fn host_marketplaces(manifest: &SignedManifest) -> Vec<HostMarketplace> {
    if manifest.plugins.is_empty() {
        return Vec::new();
    }
    manifest
        .marketplaces
        .iter()
        .map(|m| HostMarketplace {
            id: m.id.clone(),
            name: m.name.clone(),
            plugin_ids: m.plugin_ids.clone(),
            allow_cross_marketplace_dependencies_on: m
                .allow_cross_marketplace_dependencies_on
                .clone(),
            external_marketplaces: m.external_marketplaces.clone(),
            external_plugins: m.external_plugins.clone(),
        })
        .collect()
}

#[derive(Debug)]
pub struct MarketplaceEntry {
    pub name: String,
    pub description: String,
    pub version: String,
}

pub(super) fn entry_for(src: &Path, plugin_id: &PluginId, version: &str) -> MarketplaceEntry {
    MarketplaceEntry {
        name: plugin_id.as_str().to_owned(),
        description: read_plugin_description(src).unwrap_or_default(),
        version: version.to_owned(),
    }
}

fn read_plugin_description(plugin_dir: &Path) -> Option<String> {
    foreign::read_plugin_manifest(plugin_dir).map(|m| m.description)
}

pub(super) fn write_marketplace_json(
    plugins: &Path,
    marketplace: &HostMarketplace,
    version: &str,
    entries: &[MarketplaceEntry],
) -> Result<(), ApplyError> {
    let dir = marketplace_dir(plugins, &marketplace.id).join(".claude-plugin");
    fs_create(&dir)?;
    let mut value = marketplace_value(
        marketplace.id.as_str(),
        &marketplace.name,
        version,
        entries,
        &marketplace.allow_cross_marketplace_dependencies_on,
    );
    append_external_plugins(&mut value, &marketplace.external_plugins)?;
    write_json(&dir.join("marketplace.json"), &value)
}

// JSON: Claude Code `marketplace.json` — each entry is written as authored so
// Claude Code fetches the pinned commit itself.
pub fn append_external_plugins(
    catalog: &mut Value,
    external: &[ManifestExternalPlugin],
) -> Result<(), ApplyError> {
    let Some(plugins) = catalog.get_mut("plugins").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    for entry in external {
        let value = serde_json::to_value(entry).map_err(|e| ApplyError::Serialize {
            what: format!("pass-through plugin {}", entry.name),
            source: e,
        })?;
        plugins.push(value);
    }
    Ok(())
}

// Why: Claude Code requires an object owner and a manifest name matching the
// marketplace key, and refuses a cross-marketplace dependency unless this
// root marketplace allowlists the target here.
// JSON: Claude Code `marketplace.json` — foreign file format, unknown keys must
// be preserved.
#[must_use]
pub fn marketplace_value(
    marketplace: &str,
    description: &str,
    version: &str,
    entries: &[MarketplaceEntry],
    allow_cross_marketplace_dependencies_on: &[String],
) -> Value {
    let plugins: Vec<Value> = entries
        .iter()
        .map(|e| {
            json!({
                "name": e.name,
                "source": format!("./plugins/{}", e.name),
                "description": e.description,
                "version": e.version,
            })
        })
        .collect();
    let mut value = json!({
        "$schema": "https://anthropic.com/claude-code/marketplace.schema.json",
        "name": marketplace,
        "description": description,
        "owner": { "name": "systemprompt.io", "email": "support@systemprompt.io" },
        "metadata": { "version": version, "pluginRoot": "./plugins" },
        "plugins": plugins,
    });
    if !allow_cross_marketplace_dependencies_on.is_empty() {
        value["allowCrossMarketplaceDependenciesOn"] =
            json!(allow_cross_marketplace_dependencies_on);
    }
    value
}

pub fn upsert_known_marketplace(
    plugins: &Path,
    marketplace: &MarketplaceId,
    updated_at: &str,
) -> Result<(), ApplyError> {
    let path = plugins.join("known_marketplaces.json");
    let mut root = read_json_object(&path)?;
    let loc = marketplace_dir(plugins, marketplace)
        .to_string_lossy()
        .into_owned();
    root.insert(
        marketplace.as_str().to_owned(),
        json!({
            "source": { "source": "directory", "path": &loc },
            "installLocation": loc,
            "lastUpdated": updated_at,
        }),
    );
    write_json(&path, &Value::Object(root))
}

pub fn strip_known_marketplace(
    plugins: &Path,
    marketplace: &MarketplaceId,
) -> Result<(), ApplyError> {
    let path = plugins.join("known_marketplaces.json");
    let Some(mut root) = read_optional_object(&path)? else {
        return Ok(());
    };
    if root.remove(marketplace.as_str()).is_some() {
        write_json(&path, &Value::Object(root))?;
    }
    Ok(())
}

pub(super) fn set_enabled(
    current: &[Mirrored],
    stale: &[MarketplaceId],
    previous: &Owned,
    foreign: &ForeignRefs,
) -> Result<(), ApplyError> {
    let Some(path) = paths::claude_cli_settings_path() else {
        return Ok(());
    };
    let mut root = read_json_object(&path)?;

    let enabled_map = object_entry(&mut root, &path, "enabledPlugins")?;
    for marketplace in stale {
        strip_marketplace_keys(enabled_map, marketplace, &[]);
    }
    for mirrored in current {
        let ids: Vec<&str> = mirrored.plugin_ids.iter().map(PluginId::as_str).collect();
        strip_marketplace_keys(enabled_map, &mirrored.id, &ids);
        for id in &mirrored.plugin_ids {
            enabled_map.insert(plugin_key(id, &mirrored.id), Value::Bool(true));
        }
    }
    // Why: Claude Code keys a pass-through dependency as
    // `<plugin>@<marketplace>` under a mirrored marketplace, so it is written
    // after that marketplace's sweep or the sweep would take it back.
    foreign::apply_settings(&mut root, &path, previous, foreign)?;

    {
        let mkts = object_entry(&mut root, &path, "extraKnownMarketplaces")?;
        for marketplace in stale {
            mkts.remove(marketplace.as_str());
        }
        let plugins = paths::claude_cli_plugins_dir().ok_or_else(|| {
            io_err(
                "resolve the Claude Code plugins directory for extraKnownMarketplaces",
                std::io::Error::new(std::io::ErrorKind::NotFound, "no home directory"),
            )
        })?;
        for mirrored in current {
            let loc = marketplace_dir(&plugins, &mirrored.id)
                .to_string_lossy()
                .into_owned();
            mkts.insert(
                mirrored.id.as_str().to_owned(),
                json!({ "source": { "source": "directory", "path": loc } }),
            );
        }
    }

    write_json(&path, &Value::Object(root))
}

// JSON: Claude Code `marketplace.json` — foreign file format, unknown keys must
// be preserved.
pub(super) fn strip_marketplace_keys(
    map: &mut serde_json::Map<String, Value>,
    marketplace: &MarketplaceId,
    keep: &[&str],
) -> bool {
    let suffix = format!("@{}", marketplace.as_str());
    let expected: Vec<String> = keep.iter().map(|name| format!("{name}{suffix}")).collect();
    let stale: Vec<String> = map
        .keys()
        .filter(|k| k.ends_with(&suffix) && !expected.iter().any(|e| e == *k))
        .cloned()
        .collect();
    let removed = !stale.is_empty();
    for key in stale {
        map.remove(&key);
    }
    removed
}

fn fs_create(dir: &Path) -> Result<(), ApplyError> {
    std::fs::create_dir_all(dir).map_err(|e| io_err(format!("create {}", dir.display()), e))
}
