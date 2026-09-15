//! Plugins this emitter does not mirror but Claude Code must still install:
//! the `dependencies` each org plugin's `plugin.json` declares, and the
//! marketplaces those dependencies live in.
//!
//! Claude Code resolves and installs dependencies itself, but only through
//! its own install and enable paths — which the bridge bypasses by writing
//! the cache directly. So the bridge writes what those paths would have
//! written: the dependency enabled at user scope under
//! `<plugin>@<marketplace>`, and the foreign marketplace registered in
//! `extraKnownMarketplaces`. Claude Code then clones and caches the
//! dependency at its next session start.
//!
//! Every key written here is recorded in the sidecar so a later sync or
//! uninstall removes exactly these and never a key the user added.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Map, Value};
use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::bridge::manifest::ExternalMarketplace;
use systemprompt_models::bridge::plugin_bundle::PluginManifest;

use super::marketplace::HostMarketplace;
use super::sidecar::Owned;
use crate::host_sync::ApplyError;
use crate::integration::json_io::object_entry;

/// The settings entries one sync run owes Claude Code beyond the mirrored
/// marketplaces themselves.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForeignRefs {
    pub dependency_keys: BTreeSet<String>,
    pub external_marketplaces: Vec<ExternalMarketplace>,
}

impl ForeignRefs {
    pub fn extend(&mut self, other: Self) {
        self.dependency_keys.extend(other.dependency_keys);
        for external in other.external_marketplaces {
            if !self
                .external_marketplaces
                .iter()
                .any(|known| known.name == external.name)
            {
                self.external_marketplaces.push(external);
            }
        }
    }

    #[must_use]
    pub fn external_names(&self) -> Vec<String> {
        self.external_marketplaces
            .iter()
            .map(|m| m.name.clone())
            .collect()
    }
}

#[must_use]
pub fn read_plugin_manifest(plugin_dir: &Path) -> Option<PluginManifest> {
    use systemprompt_models::bridge::plugin_bundle::{PLUGIN_MANIFEST_DIRS, PLUGIN_MANIFEST_FILE};
    let path = PLUGIN_MANIFEST_DIRS
        .iter()
        .map(|dir| plugin_dir.join(dir).join(PLUGIN_MANIFEST_FILE))
        .find(|p| p.is_file())?;
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

// Why: a dependency on a marketplace this run mirrors is already enabled by
// the mirror itself, so only keys that leave the mirrored set are collected.
#[must_use]
pub fn collect(
    marketplace: &HostMarketplace,
    plugin_dirs: &[&Path],
    mirrored: &[MarketplaceId],
) -> ForeignRefs {
    let mut refs = ForeignRefs {
        dependency_keys: BTreeSet::new(),
        external_marketplaces: marketplace.external_marketplaces.clone(),
    };
    for dir in plugin_dirs {
        let Some(manifest) = read_plugin_manifest(dir) else {
            continue;
        };
        for dependency in &manifest.dependencies {
            let target = dependency.marketplace().unwrap_or(marketplace.id.as_str());
            if mirrored.iter().any(|id| id.as_str() == target) {
                continue;
            }
            refs.dependency_keys
                .insert(format!("{}@{target}", dependency.name()));
        }
    }
    refs
}

pub fn apply_settings(
    root: &mut Map<String, Value>,
    path: &Path,
    previous: &Owned,
    current: &ForeignRefs,
) -> Result<(), ApplyError> {
    let enabled = object_entry(root, path, "enabledPlugins")?;
    for key in &previous.dependency_keys {
        if !current.dependency_keys.contains(key) {
            enabled.remove(key);
        }
    }
    for key in &current.dependency_keys {
        enabled.insert(key.clone(), Value::Bool(true));
    }

    let known = object_entry(root, path, "extraKnownMarketplaces")?;
    for name in &previous.external_marketplaces {
        if !current
            .external_marketplaces
            .iter()
            .any(|m| &m.name == name)
        {
            known.remove(name);
        }
    }
    for external in &current.external_marketplaces {
        let source = serde_json::to_value(&external.source).map_err(|e| ApplyError::Serialize {
            what: format!("external marketplace {}", external.name),
            source: e,
        })?;
        known.insert(
            external.name.clone(),
            serde_json::json!({ "source": source }),
        );
    }
    Ok(())
}
