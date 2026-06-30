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
//!
//! [`plugin_bundles_cached`] memoizes that map keyed by a fingerprint of the
//! inputs. A bridge "sync" fetches one manifest and then one request per plugin
//! file; without the cache each file request would reassemble every bundle. The
//! cached value is exactly what `plugin_bundles` would return, so the
//! single-source guarantee is unchanged — reassembly happens only when the
//! services config or resolved catalogue content changes.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock, RwLock};

use sha2::{Digest, Sha256};
use systemprompt_models::bridge::ids::{PluginId, Sha256Digest};
use systemprompt_models::bridge::manifest::{PluginEntry, PluginFile};
use systemprompt_models::services::{PluginConfig, ServicesConfig};

use crate::bundle::{BundleContent, PluginBundle, build_plugin_bundle, bundle_has_content};
use crate::catalog::fingerprint::hash_dir_metadata;
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

type BundleMap = BTreeMap<PluginId, PluginBundle>;
type BundleCache = OnceLock<RwLock<Option<([u8; 32], Arc<BundleMap>)>>>;

static BUNDLE_CACHE: BundleCache = OnceLock::new();

/// Memoized [`plugin_bundles`]: returns the cached map when the fingerprint of
/// `services` and `content` is unchanged, otherwise reassembles and caches it.
pub fn plugin_bundles_cached(
    services: &ServicesConfig,
    content: &BundleContent<'_>,
) -> Result<Arc<BundleMap>, MarketplaceError> {
    let fingerprint = bundle_fingerprint(services, content)?;
    let cache = BUNDLE_CACHE.get_or_init(|| RwLock::new(None));

    let hit = {
        let guard = cache
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard
            .as_ref()
            .filter(|(cached_fp, _)| *cached_fp == fingerprint)
            .map(|(_, bundles)| Arc::clone(bundles))
    };
    if let Some(bundles) = hit {
        return Ok(bundles);
    }

    let bundles = Arc::new(plugin_bundles(services, content)?);
    let mut guard = cache
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = Some((fingerprint, Arc::clone(&bundles)));
    drop(guard);
    Ok(bundles)
}

fn bundle_fingerprint(
    services: &ServicesConfig,
    content: &BundleContent<'_>,
) -> Result<[u8; 32], MarketplaceError> {
    let mut hasher = Sha256::new();
    hash_part(&mut hasher, &to_json(services)?);
    hash_part(&mut hasher, &to_json(content.skills)?);
    hash_part(&mut hasher, &to_json(content.agents)?);
    hash_part(&mut hasher, &to_json(content.mcp_servers)?);
    hash_part(&mut hasher, &to_json(content.disabled_mcp_servers)?);
    hash_part(
        &mut hasher,
        content.plugins_root.as_os_str().as_encoded_bytes(),
    );
    hash_dir_metadata(&mut hasher, content.plugins_root);
    Ok(hasher.finalize().into())
}

fn to_json<T: serde::Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, MarketplaceError> {
    serde_json::to_vec(value).map_err(|e| MarketplaceError::Catalog(e.to_string()))
}

fn hash_part(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
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

    let bundles = plugin_bundles_cached(services, content)?;
    let mut out = Vec::with_capacity(bundles.len());
    for (id, bundle) in bundles.iter() {
        let version = versions.get(id.as_str()).copied().ok_or_else(|| {
            MarketplaceError::Catalog(format!("plugin {id} missing from services config"))
        })?;
        out.push(hash_entry(id.clone(), version, bundle)?);
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
