//! Marketplace discovery and the user's plugin registry.
//!
//! Covers `marketplace.json`, `known_marketplaces.json`,
//! `installed_plugins.json`, and the `settings.json` enablement entries. Every
//! registry file is updated in place so the user's own marketplaces and plugins
//! survive untouched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::{Value, json};
use systemprompt_models::bridge::plugin_bundle::PluginManifest;

use super::json_io::{object_entry, read_json_object, read_optional_object, write_json};
use super::{MARKETPLACE, cache_install_dir, io_err, marketplace_dir, plugin_key};
use crate::config::paths;
use crate::gateway::manifest::SignedManifest;
use crate::sync::ApplyError;

#[derive(Debug)]
pub struct MarketplaceEntry {
    pub name: String,
    pub description: String,
    pub version: String,
}

pub(super) fn entry_for(src: &Path, plugin_id: &str, version: &str) -> MarketplaceEntry {
    MarketplaceEntry {
        name: plugin_id.to_owned(),
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
    version: &str,
    entries: &[MarketplaceEntry],
) -> Result<(), ApplyError> {
    let dir = marketplace_dir(plugins).join(".claude-plugin");
    fs_create(&dir)?;
    write_json(
        &dir.join("marketplace.json"),
        &marketplace_value(version, entries),
    )
}

// `owner` is a required object and `name` must equal the marketplace key, or
// `claude plugin validate` rejects the manifest ("owner: expected object").
pub fn marketplace_value(version: &str, entries: &[MarketplaceEntry]) -> Value {
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
        "name": MARKETPLACE,
        "description": "Skills, agents, and MCP servers provisioned by your organization.",
        "owner": { "name": "systemprompt.io", "email": "support@systemprompt.io" },
        "metadata": { "version": version, "pluginRoot": "./plugins" },
        "plugins": plugins,
    })
}

pub fn upsert_known_marketplace(plugins: &Path, updated_at: &str) -> Result<(), ApplyError> {
    let path = plugins.join("known_marketplaces.json");
    let mut root = read_json_object(&path)?;
    let loc = marketplace_dir(plugins).to_string_lossy().into_owned();
    root.insert(
        MARKETPLACE.to_owned(),
        json!({
            "source": { "source": "directory", "path": &loc },
            "installLocation": loc,
            "lastUpdated": updated_at,
        }),
    );
    write_json(&path, &Value::Object(root))
}

pub fn strip_known_marketplace(plugins: &Path) -> Result<(), ApplyError> {
    let path = plugins.join("known_marketplaces.json");
    let Some(mut root) = read_optional_object(&path)? else {
        return Ok(());
    };
    if root.remove(MARKETPLACE).is_some() {
        write_json(&path, &Value::Object(root))?;
    }
    Ok(())
}

pub(super) fn upsert_installed_plugins(
    plugins: &Path,
    manifest: &SignedManifest,
    ids: &[&str],
) -> Result<(), ApplyError> {
    let path = plugins.join("installed_plugins.json");
    let mut root = read_json_object(&path)?;
    root.entry("version").or_insert(json!(2));
    let Some(map) = object_entry(&mut root, "plugins") else {
        return Ok(());
    };
    strip_marketplace_keys(map, ids);
    for id in ids {
        map.insert(
            plugin_key(id),
            installed_entry(
                &cache_install_dir(plugins, id),
                manifest.manifest_version.as_str(),
                manifest.issued_at.as_str(),
            ),
        );
    }
    write_json(&path, &Value::Object(root))
}

pub fn installed_entry(cache: &Path, version: &str, issued_at: &str) -> Value {
    json!([{
        "scope": "user",
        "installPath": cache.to_string_lossy().into_owned(),
        "version": version,
        "installedAt": issued_at,
        "lastUpdated": issued_at,
    }])
}

pub(super) fn strip_installed_plugins(plugins: &Path) -> Result<(), ApplyError> {
    let path = plugins.join("installed_plugins.json");
    let Some(mut root) = read_optional_object(&path)? else {
        return Ok(());
    };
    let removed = root
        .get_mut("plugins")
        .and_then(Value::as_object_mut)
        .is_some_and(|m| strip_marketplace_keys(m, &[]));
    if removed {
        write_json(&path, &Value::Object(root))?;
    }
    Ok(())
}

pub(super) fn set_enabled(ids: &[&str]) -> Result<(), ApplyError> {
    let Some(path) = paths::claude_cli_settings_path() else {
        return Ok(());
    };
    let mut root = read_json_object(&path)?;

    if let Some(enabled_map) = object_entry(&mut root, "enabledPlugins") {
        strip_marketplace_keys(enabled_map, ids);
        for id in ids {
            enabled_map.insert(plugin_key(id), Value::Bool(true));
        }
    }

    if ids.is_empty() {
        if let Some(Value::Object(mkts)) = root.get_mut("extraKnownMarketplaces") {
            mkts.remove(MARKETPLACE);
        }
    } else {
        let loc = paths::claude_cli_plugins_dir()
            .map(|p| marketplace_dir(&p).to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Some(mkts) = object_entry(&mut root, "extraKnownMarketplaces") {
            mkts.insert(
                MARKETPLACE.to_owned(),
                json!({ "source": { "source": "directory", "path": loc } }),
            );
        }
    }

    write_json(&path, &Value::Object(root))
}

/// Removes every `@{MARKETPLACE}` key not in `keep`; foreign keys survive.
fn strip_marketplace_keys(map: &mut serde_json::Map<String, Value>, keep: &[&str]) -> bool {
    let suffix = format!("@{MARKETPLACE}");
    let expected: Vec<String> = keep.iter().map(|id| plugin_key(id)).collect();
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
