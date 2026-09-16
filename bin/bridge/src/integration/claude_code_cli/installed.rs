//! `installed_plugins.json`: the user-scoped install record Claude Code keeps
//! for every cached plugin, updated in place per marketplace so the user's
//! own installs survive untouched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::{Value, json};
use systemprompt_identifiers::MarketplaceId;

use super::layout::{cache_install_dir, plugin_key};
use super::marketplace::strip_marketplace_keys;
use crate::gateway::manifest::SignedManifest;
use crate::host_sync::ApplyError;
use crate::ids::PluginId;
use crate::integration::json_io::{
    object_entry, read_json_object, read_optional_object, write_json,
};

pub(super) fn upsert_installed_plugins(
    plugins: &Path,
    manifest: &SignedManifest,
    marketplace: &MarketplaceId,
    ids: &[&PluginId],
) -> Result<(), ApplyError> {
    let path = plugins.join("installed_plugins.json");
    let mut root = read_json_object(&path)?;
    root.entry("version").or_insert(json!(2));
    let map = object_entry(&mut root, &path, "plugins")?;
    strip_marketplace_keys(map, marketplace, ids);
    for id in ids {
        map.insert(
            plugin_key(id, marketplace),
            installed_entry(
                &cache_install_dir(plugins, marketplace, id),
                manifest.manifest_version.as_str(),
                &manifest.issued_at.to_rfc3339(),
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
