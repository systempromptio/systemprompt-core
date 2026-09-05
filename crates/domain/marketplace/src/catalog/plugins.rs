//! Projects plugin specs into the bundles the gateway serves and the
//! `PluginEntry` records the signed manifest carries.
//!
//! [`plugin_bundles`] is the single source: it selects the enabled plugins any
//! enabled marketplace includes, assembles each into its installable
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
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, OnceLock, RwLock};

use sha2::{Digest, Sha256};
use systemprompt_models::bridge::ids::{LibraryArtifactId, PluginId, Sha256Digest, SkillId};
use systemprompt_models::bridge::manifest::{ArtifactEntry, PluginEntry, PluginFile};
use systemprompt_models::services::{
    ComponentSource, MarketplaceMemberKind, PluginConfig, ServicesConfig,
};

use crate::bundle::{BundleContent, PluginBundle, build_plugin_bundle, bundle_has_content};
use crate::catalog::fingerprint::hash_dir_metadata;
use crate::error::MarketplaceError;
use crate::scope::{enabled_marketplaces, scope_to_union, union_include};

pub fn plugin_bundles(
    services: &ServicesConfig,
    content: &BundleContent<'_>,
) -> Result<BTreeMap<PluginId, PluginBundle>, MarketplaceError> {
    let mut out = BTreeMap::new();
    for config in selected_configs(services)? {
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
    hash_part(&mut hasher, &to_json(content.artifacts)?);
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
    let configs: HashMap<&str, &PluginConfig> = services
        .plugins
        .values()
        .map(|p| (p.id.as_str(), p))
        .collect();

    let bundles = plugin_bundles_cached(services, content)?;
    let mut out = Vec::with_capacity(bundles.len());
    for (id, bundle) in bundles.iter() {
        let config = configs.get(id.as_str()).copied().ok_or_else(|| {
            MarketplaceError::Catalog(format!("plugin {id} missing from services config"))
        })?;
        out.push(hash_entry(id.clone(), config, bundle)?);
    }
    Ok(out)
}

pub fn artifact_owners(
    services: &ServicesConfig,
    artifacts: &[ArtifactEntry],
) -> Result<BTreeMap<LibraryArtifactId, BTreeSet<PluginId>>, MarketplaceError> {
    let mut out: BTreeMap<LibraryArtifactId, BTreeSet<PluginId>> = BTreeMap::new();
    for config in selected_configs(services)? {
        let selected: Vec<LibraryArtifactId> = match config.artifacts.source {
            ComponentSource::Explicit => config
                .artifacts
                .include
                .iter()
                .map(|id| {
                    LibraryArtifactId::try_new(id.as_str())
                        .map_err(|e| MarketplaceError::Catalog(e.to_string()))
                })
                .collect::<Result<_, _>>()?,
            ComponentSource::Instance => artifacts
                .iter()
                .filter(|a| selects_artifact(config, &a.id))
                .map(|a| a.id.clone())
                .collect(),
        };
        let owner = PluginId::try_new(config.id.as_str())
            .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
        for id in selected {
            out.entry(id).or_default().insert(owner.clone());
        }
    }
    Ok(out)
}

#[must_use]
pub fn selects_artifact(config: &PluginConfig, artifact_id: &LibraryArtifactId) -> bool {
    let artifact_id = artifact_id.as_str();
    match config.artifacts.source {
        ComponentSource::Explicit => config
            .artifacts
            .include
            .iter()
            .any(|inc| inc == artifact_id),
        ComponentSource::Instance => !config.artifacts.exclude.iter().any(|ex| ex == artifact_id),
    }
}

pub(crate) fn selected_configs(
    services: &ServicesConfig,
) -> Result<Vec<&PluginConfig>, MarketplaceError> {
    let enabled: Vec<&PluginConfig> = services.plugins.values().filter(|p| p.enabled).collect();
    let marketplaces = enabled_marketplaces(services);
    let include = union_include(&marketplaces, MarketplaceMemberKind::Plugins);
    let mut scoped = scope_to_union(enabled, include.as_ref(), |c| c.id.as_str());
    scoped.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    Ok(scoped)
}

pub fn skill_owners(
    services: &ServicesConfig,
    content: &BundleContent<'_>,
) -> Result<BTreeMap<SkillId, BTreeSet<PluginId>>, MarketplaceError> {
    let mut out: BTreeMap<SkillId, BTreeSet<PluginId>> = BTreeMap::new();
    for config in selected_configs(services)? {
        let agent_ids = crate::bundle::resolve_agents(config, content.agents);
        let owner = PluginId::try_new(config.id.as_str())
            .map_err(|e| MarketplaceError::Catalog(e.to_string()))?;
        for id in crate::bundle::resolve_skill_ids(config, content, &agent_ids) {
            out.entry(id).or_default().insert(owner.clone());
        }
    }
    Ok(out)
}

fn hash_entry(
    id: PluginId,
    config: &PluginConfig,
    bundle: &PluginBundle,
) -> Result<PluginEntry, MarketplaceError> {
    let version = config.version.as_str();
    let mut hasher = Sha256::new();
    hasher.update(id.as_str().as_bytes());
    hasher.update(version.as_bytes());
    hasher.update(&to_json(&config.hooks)?);

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
        hooks: config.hooks.clone(),
    })
}

fn file_digest(bytes: &[u8]) -> Result<Sha256Digest, MarketplaceError> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Sha256Digest::try_new(hex::encode(hasher.finalize()))
        .map_err(|e| MarketplaceError::Catalog(e.to_string()))
}
