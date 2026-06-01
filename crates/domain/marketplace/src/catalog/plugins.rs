//! Projects plugin specs into the signed `PluginEntry` records the manifest
//! carries.
//!
//! Each enabled [`PluginConfig`] is assembled into its installable bundle via
//! [`build_plugin_bundle`] (the single owner of the bundle contract) and the
//! generated files are hashed into a [`PluginEntry`]. A spec whose references
//! resolve to no content is skipped fail-closed — an empty shell is never
//! minted into the signed manifest.

use sha2::{Digest, Sha256};
use systemprompt_models::bridge::ids::{PluginId, Sha256Digest};
use systemprompt_models::bridge::manifest::{PluginEntry, PluginFile};
use systemprompt_models::services::{PluginConfig, ServicesConfig};

use crate::bundle::{BundleContent, build_plugin_bundle, bundle_has_content};
use crate::error::MarketplaceError;

#[must_use]
pub fn load_plugins(services: &ServicesConfig, content: &BundleContent<'_>) -> Vec<PluginEntry> {
    let mut configs: Vec<&PluginConfig> = services.plugins.values().filter(|p| p.enabled).collect();
    configs.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

    let mut out = Vec::with_capacity(configs.len());
    for config in configs {
        match build_plugin_entry(config, content) {
            Ok(Some(entry)) => out.push(entry),
            Ok(None) => {
                tracing::warn!(
                    plugin_id = %config.id,
                    "manifest: plugin references resolve to no content; skipping"
                );
            },
            Err(e) => {
                tracing::warn!(
                    plugin_id = %config.id,
                    error = %e,
                    "manifest: failed to assemble plugin bundle; skipping"
                );
            },
        }
    }
    out
}

fn build_plugin_entry(
    config: &PluginConfig,
    content: &BundleContent<'_>,
) -> Result<Option<PluginEntry>, MarketplaceError> {
    let bundle = build_plugin_bundle(config, content)?;
    if !bundle_has_content(&bundle) {
        return Ok(None);
    }

    let mut hasher = Sha256::new();
    hasher.update(config.id.as_str().as_bytes());
    hasher.update(config.version.as_bytes());

    let mut files = Vec::with_capacity(bundle.len());
    for (path, file) in &bundle {
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
    let id = PluginId::try_new(config.id.as_str())
        .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
    Ok(Some(PluginEntry {
        id,
        version: config.version.clone(),
        sha256: aggregate,
        files,
    }))
}

fn file_digest(bytes: &[u8]) -> Result<Sha256Digest, MarketplaceError> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Sha256Digest::try_new(hex::encode(hasher.finalize()))
        .map_err(|e| MarketplaceError::Catalog(e.to_string()))
}
