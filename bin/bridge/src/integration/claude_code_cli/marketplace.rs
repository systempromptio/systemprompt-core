//! Marketplace discovery and the user's plugin registry.
//!
//! Covers `marketplace.json`, `known_marketplaces.json`,
//! `installed_plugins.json`, and the `settings.json` enablement entries. Every
//! registry file is updated in place so the user's own marketplaces and plugins
//! survive untouched.

use std::path::Path;

use serde_json::{Value, json};

use super::json_io::{object_entry, read_json_object, read_optional_object, write_json};
use super::{MARKETPLACE, PLUGIN_NAME, io_err, marketplace_dir, plugin_id};
use crate::config::paths;
use crate::gateway::manifest::SignedManifest;
use crate::sync::ApplyError;

pub(super) fn write_marketplace_json(
    plugins: &Path,
    manifest: &SignedManifest,
) -> Result<(), ApplyError> {
    let dir = marketplace_dir(plugins).join(".claude-plugin");
    fs_create(&dir)?;
    write_json(
        &dir.join("marketplace.json"),
        &marketplace_value(manifest.manifest_version.as_str()),
    )
}

// `owner` is a required object and `name` must equal the marketplace key, or
// `claude plugin validate` rejects the manifest ("owner: expected object").
pub fn marketplace_value(version: &str) -> Value {
    json!({
        "$schema": "https://anthropic.com/claude-code/marketplace.schema.json",
        "name": MARKETPLACE,
        "description": "Skills, agents, and MCP servers provisioned by your systemprompt.io organization.",
        "owner": { "name": "systemprompt.io", "email": "support@systemprompt.io" },
        "metadata": { "version": version, "pluginRoot": "./plugins" },
        "plugins": [{
            "name": PLUGIN_NAME,
            "source": format!("./plugins/{PLUGIN_NAME}"),
            "description": "Skills, agents, and MCP servers managed by your organization.",
            "version": version,
        }],
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

pub(super) fn upsert_installed_plugin(
    plugins: &Path,
    cache: &Path,
    manifest: &SignedManifest,
) -> Result<(), ApplyError> {
    let path = plugins.join("installed_plugins.json");
    let mut root = read_json_object(&path)?;
    root.entry("version").or_insert(json!(2));
    let Some(map) = object_entry(&mut root, "plugins") else {
        return Ok(());
    };
    map.insert(
        plugin_id(),
        installed_entry(
            cache,
            manifest.manifest_version.as_str(),
            manifest.issued_at.as_str(),
        ),
    );
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

pub(super) fn strip_installed_plugin(plugins: &Path) -> Result<(), ApplyError> {
    let path = plugins.join("installed_plugins.json");
    let Some(mut root) = read_optional_object(&path)? else {
        return Ok(());
    };
    let removed = root
        .get_mut("plugins")
        .and_then(Value::as_object_mut)
        .is_some_and(|m| m.remove(&plugin_id()).is_some());
    if removed {
        write_json(&path, &Value::Object(root))?;
    }
    Ok(())
}

pub(super) fn set_enabled(enabled: bool) -> Result<(), ApplyError> {
    let Some(path) = paths::claude_cli_settings_path() else {
        return Ok(());
    };
    let mut root = read_json_object(&path)?;

    if let Some(enabled_map) = object_entry(&mut root, "enabledPlugins") {
        if enabled {
            enabled_map.insert(plugin_id(), Value::Bool(true));
        } else {
            enabled_map.remove(&plugin_id());
        }
    }

    if enabled {
        let loc = paths::claude_cli_plugins_dir()
            .map(|p| marketplace_dir(&p).to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Some(mkts) = object_entry(&mut root, "extraKnownMarketplaces") {
            mkts.insert(
                MARKETPLACE.to_owned(),
                json!({ "source": { "source": "directory", "path": loc } }),
            );
        }
    } else if let Some(Value::Object(mkts)) = root.get_mut("extraKnownMarketplaces") {
        mkts.remove(MARKETPLACE);
    }

    write_json(&path, &Value::Object(root))
}

fn fs_create(dir: &Path) -> Result<(), ApplyError> {
    std::fs::create_dir_all(dir).map_err(|e| io_err(format!("create {}", dir.display()), e))
}
