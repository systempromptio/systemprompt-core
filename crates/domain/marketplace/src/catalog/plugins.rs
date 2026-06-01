//! Projects plugin specs into the bundles the gateway serves and the
//! `PluginEntry` records the signed manifest carries.
//!
//! [`plugin_bundles`] is the single source: it selects the enabled,
//! active-marketplace-scoped plugins, assembles each into its installable
//! bundle via [`build_plugin_bundle`] (the owner of the bundle contract), and
//! drops fail-closed any spec whose references resolve to no content. Both the
//! manifest path ([`load_plugins`], which hashes each bundle into a
//! [`PluginEntry`]) and the gateway byte-serving path consume that one map, so
//! the hashed entry and the streamed bytes cannot drift.

use std::collections::{BTreeMap, HashMap};

use sha2::{Digest, Sha256};
use systemprompt_models::bridge::ids::{PluginId, Sha256Digest};
use systemprompt_models::bridge::manifest::{PluginEntry, PluginFile};
use systemprompt_models::services::{PluginConfig, ServicesConfig};

use crate::bundle::{BundleContent, PluginBundle, build_plugin_bundle, bundle_has_content};
use crate::error::MarketplaceError;
use crate::scope::{active_marketplace, scope_to_marketplace};

pub fn plugin_bundles(
    services: &ServicesConfig,
    content: &BundleContent<'_>,
) -> Result<BTreeMap<PluginId, PluginBundle>, MarketplaceError> {
    let mut out = BTreeMap::new();
    for config in selected_configs(services) {
        let bundle = match build_plugin_bundle(config, content) {
            Ok(bundle) => bundle,
            Err(e) => {
                tracing::warn!(
                    plugin_id = %config.id,
                    error = %e,
                    "marketplace: failed to assemble plugin bundle; skipping"
                );
                continue;
            },
        };
        if !bundle_has_content(&bundle) {
            tracing::warn!(
                plugin_id = %config.id,
                "marketplace: plugin references resolve to no content; skipping"
            );
            continue;
        }
        let id = PluginId::try_new(config.id.as_str())
            .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
        out.insert(id, bundle);
    }
    Ok(out)
}

pub fn load_plugins(
    services: &ServicesConfig,
    content: &BundleContent<'_>,
) -> Result<Vec<PluginEntry>, MarketplaceError> {
    let versions: HashMap<&str, &str> = services
        .plugins
        .values()
        .map(|p| (p.id.as_str(), p.version.as_str()))
        .collect();

    let bundles = plugin_bundles(services, content)?;
    let mut out = Vec::with_capacity(bundles.len());
    for (id, bundle) in bundles {
        let version = versions.get(id.as_str()).copied().ok_or_else(|| {
            MarketplaceError::Catalog(format!("plugin {id} missing from services config"))
        })?;
        out.push(hash_entry(id, version, &bundle)?);
    }
    Ok(out)
}

fn selected_configs(services: &ServicesConfig) -> Vec<&PluginConfig> {
    let enabled: Vec<&PluginConfig> = services.plugins.values().filter(|p| p.enabled).collect();
    let mut scoped = match active_marketplace(services) {
        Some(mp) => scope_to_marketplace(enabled, &mp.plugins.include, |c| c.id.as_str()),
        None => enabled,
    };
    scoped.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    scoped
}

fn hash_entry(
    id: PluginId,
    version: &str,
    bundle: &PluginBundle,
) -> Result<PluginEntry, MarketplaceError> {
    let mut hasher = Sha256::new();
    hasher.update(id.as_str().as_bytes());
    hasher.update(version.as_bytes());

    let mut files = Vec::with_capacity(bundle.len());
    for (path, file) in bundle {
        let sha256 = file_digest(&file.bytes)?;
        hasher.update(path.as_bytes());
        hasher.update(sha256.as_str().as_bytes());
        files.push(PluginFile {
            path: path.clone(),
            sha256,
            size: file.bytes.len() as u64,
        });
    }

    let aggregate = Sha256Digest::try_new(hex::encode(hasher.finalize()))
        .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    Ok(PluginEntry {
        id,
        version: version.to_owned(),
        sha256: aggregate,
        files,
    })
}

fn file_digest(bytes: &[u8]) -> Result<Sha256Digest, MarketplaceError> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Sha256Digest::try_new(hex::encode(hasher.finalize()))
        .map_err(|e| MarketplaceError::Catalog(e.to_string()))
}
