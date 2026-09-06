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
use systemprompt_models::bridge::plugin_bundle::PluginManifest;

use super::json_io::{object_entry, read_json_object, read_optional_object, write_json};
use super::{HostMarketplace, Mirrored, cache_install_dir, io_err, marketplace_dir, plugin_key};
use crate::config::paths;
use crate::gateway::manifest::SignedManifest;
use crate::host_sync::ApplyError;
use crate::ids::PluginId;

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
    use systemprompt_models::bridge::plugin_bundle::{PLUGIN_MANIFEST_DIRS, PLUGIN_MANIFEST_FILE};
    let path = PLUGIN_MANIFEST_DIRS
        .iter()
        .map(|dir| plugin_dir.join(dir).join(PLUGIN_MANIFEST_FILE))
        .find(|p| p.is_file())?;
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice::<PluginManifest>(&bytes)
        .ok()
        .map(|m| m.description)
}

pub(super) fn write_marketplace_json(
    plugins: &Path,
    marketplace: &HostMarketplace,
    version: &str,
    entries: &[MarketplaceEntry],
) -> Result<(), ApplyError> {
    let dir = marketplace_dir(plugins, &marketplace.id).join(".claude-plugin");
    fs_create(&dir)?;
    write_json(
        &dir.join("marketplace.json"),
        &marketplace_value(marketplace.id.as_str(), &marketplace.name, version, entries),
    )
}

// Why: `claude plugin validate` requires `owner` to be an object and `name` to
// equal the marketplace key, else it rejects the manifest.
#[must_use]
pub fn marketplace_value(
    marketplace: &str,
    description: &str,
    version: &str,
    entries: &[MarketplaceEntry],
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
    json!({
        "$schema": "https://anthropic.com/claude-code/marketplace.schema.json",
        "name": marketplace,
        "description": description,
        "owner": { "name": "systemprompt.io", "email": "support@systemprompt.io" },
        "metadata": { "version": version, "pluginRoot": "./plugins" },
        "plugins": plugins,
    })
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

pub(super) fn upsert_installed_plugins(
    plugins: &Path,
    manifest: &SignedManifest,
    marketplace: &MarketplaceId,
    ids: &[&PluginId],
) -> Result<(), ApplyError> {
    let path = plugins.join("installed_plugins.json");
    let mut root = read_json_object(&path)?;
    root.entry("version").or_insert(json!(2));
    let Some(map) = object_entry(&mut root, "plugins") else {
        return Ok(());
    };
    strip_marketplace_keys(map, marketplace, ids);
    for id in ids {
        map.insert(
            plugin_key(id, marketplace),
            installed_entry(
                &cache_install_dir(plugins, marketplace, id),
                manifest.manifest_version.as_str(),
                manifest.issued_at.as_str(),
            ),
        );
    }
    write_json(&path, &Value::Object(root))
}

#[must_use]
pub fn installed_entry(cache: &Path, version: &str, issued_at: &str) -> Value {
    json!([{
        "scope": "user",
        "installPath": cache.to_string_lossy().into_owned(),
        "version": version,
        "installedAt": issued_at,
        "lastUpdated": issued_at,
    }])
}

pub fn strip_installed_plugins(
    plugins: &Path,
    marketplace: &MarketplaceId,
) -> Result<(), ApplyError> {
    let path = plugins.join("installed_plugins.json");
    let Some(mut root) = read_optional_object(&path)? else {
        return Ok(());
    };
    let removed = root
        .get_mut("plugins")
        .and_then(Value::as_object_mut)
        .is_some_and(|m| strip_marketplace_keys(m, marketplace, &[]));
    if removed {
        write_json(&path, &Value::Object(root))?;
    }
    Ok(())
}

// Why: `stale` names marketplaces this emitter wrote before and no longer
// mirrors; only their keys are stripped, so an enable the user set for a
// marketplace of their own is never touched.
pub(super) fn set_enabled(current: &[Mirrored], stale: &[MarketplaceId]) -> Result<(), ApplyError> {
    let Some(path) = paths::claude_cli_settings_path() else {
        return Ok(());
    };
    let mut root = read_json_object(&path)?;

    if let Some(enabled_map) = object_entry(&mut root, "enabledPlugins") {
        for marketplace in stale {
            strip_marketplace_keys(enabled_map, marketplace, &[]);
        }
        for mirrored in current {
            let ids: Vec<&PluginId> = mirrored.plugin_ids.iter().collect();
            strip_marketplace_keys(enabled_map, &mirrored.id, &ids);
            for id in ids {
                enabled_map.insert(plugin_key(id, &mirrored.id), Value::Bool(true));
            }
        }
    }

    if let Some(mkts) = object_entry(&mut root, "extraKnownMarketplaces") {
        for marketplace in stale {
            mkts.remove(marketplace.as_str());
        }
        let plugins = paths::claude_cli_plugins_dir();
        for mirrored in current {
            let loc = plugins
                .as_deref()
                .map(|p| {
                    marketplace_dir(p, &mirrored.id)
                        .to_string_lossy()
                        .into_owned()
                })
                .unwrap_or_default();
            mkts.insert(
                mirrored.id.as_str().to_owned(),
                json!({ "source": { "source": "directory", "path": loc } }),
            );
        }
    }

    write_json(&path, &Value::Object(root))
}

fn strip_marketplace_keys(
    map: &mut serde_json::Map<String, Value>,
    marketplace: &MarketplaceId,
    keep: &[&PluginId],
) -> bool {
    let suffix = format!("@{}", marketplace.as_str());
    let expected: Vec<String> = keep.iter().map(|id| plugin_key(id, marketplace)).collect();
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
