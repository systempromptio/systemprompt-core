//! Where the active services tree came from, as a value a projection can
//! store and a page can render.
//!
//! The sync page reads the same three things every time — the composed hash
//! the instance booted, the hash of the repository's own `services/` tree,
//! and each pinned bundle's digest, version and content hash. Recording them
//! beside an inventory observation is what turns a generation number into
//! provenance: the reader can say which declarations produced it. Nothing
//! here fetches; every value comes from the bundle cache's `state.json`, the
//! cached `bundle.json`, and the services root installed at boot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use systemprompt_config::ProfileBootstrap;
use systemprompt_models::services::bundle::{BUNDLE_ALLOWED_DIRS, ServicesBundleManifest};

use super::bootstrap::cache_root;
use super::cache::BundleCache;
use crate::services_root::{ServicesProvenance, ServicesRootBootstrap};

/// One pinned bundle as the cache last recorded it. `pinned_digest` is what
/// the profile asks for and `active_digest` is what was fetched; they differ
/// while a re-pin is waiting for a restart.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BundleProvenance {
    pub name: String,
    pub pinned_digest: Option<String>,
    pub active_digest: Option<String>,
    pub content_hash: Option<String>,
    pub version: Option<String>,
}

/// The whole declaration surface behind one observation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SourcesProvenance {
    pub composed_hash: Option<String>,
    pub base_tree_hash: Option<String>,
    pub bundles: Vec<BundleProvenance>,
}

#[must_use]
pub fn sources_provenance() -> SourcesProvenance {
    let profile = match ProfileBootstrap::get() {
        Ok(profile) => profile,
        Err(error) => {
            tracing::warn!(%error, "No bootstrapped profile; recording empty sources provenance");
            return SourcesProvenance::default();
        },
    };
    let cache = BundleCache::new(cache_root(profile));
    let state = cache.read_state();
    let bundles: Vec<BundleProvenance> = profile
        .services
        .sources
        .iter()
        .map(|source| {
            let active = state.sources.get(&source.name);
            let reference = source.oci.as_ref().map_or_else(
                || {
                    source
                        .https
                        .as_ref()
                        .map(|h| h.url.clone())
                        .unwrap_or_default()
                },
                |oci| oci.reference.clone(),
            );
            BundleProvenance {
                name: source.name.clone(),
                pinned_digest: reference
                    .split_once("@sha256:")
                    .map(|(_, digest)| format!("sha256:{digest}")),
                active_digest: active.map(|a| a.digest.clone()),
                content_hash: active.map(|a| a.content_hash.clone()),
                version: active.map(|a| a.version.clone()),
            }
        })
        .collect();
    let base = base_tree_hash(Path::new(&profile.paths.services));
    SourcesProvenance {
        // Why: with no bundles pinned no boot records a composed hash; the
        // composition is the base tree alone.
        composed_hash: active_composed_hash()
            .or_else(|| (!state.composed_hash.is_empty()).then(|| state.composed_hash.clone()))
            .or_else(|| bundles.is_empty().then(|| base.clone()).flatten()),
        base_tree_hash: base,
        bundles,
    }
}

// Why: keyed by resource name, not by bundle name — the caller has a skill in
// hand and needs the kit that declares it, which only the cached manifest
// knows.
#[must_use]
pub fn owning_bundle_hashes() -> BTreeMap<String, String> {
    let profile = match ProfileBootstrap::get() {
        Ok(profile) => profile,
        Err(error) => {
            tracing::warn!(%error, "No bootstrapped profile; no bundle owns any skill");
            return BTreeMap::new();
        },
    };
    let cache = BundleCache::new(cache_root(profile));
    let state = cache.read_state();
    let mut owners = BTreeMap::new();
    for source in &profile.services.sources {
        let Some(active) = state.sources.get(&source.name) else {
            continue;
        };
        let signed = match cache.read_manifest(&source.name, &active.content_hash) {
            Ok(signed) => signed,
            Err(error) => {
                tracing::warn!(source = %source.name, %error, "Cached bundle manifest unreadable; its skills carry no bundle hash");
                continue;
            },
        };
        for skill in &signed.manifest.owns.skills {
            owners.insert(skill.clone(), active.content_hash.clone());
        }
    }
    owners
}

fn active_composed_hash() -> Option<String> {
    match ServicesRootBootstrap::get().map(|root| &root.provenance) {
        Some(
            ServicesProvenance::Fetched { composed_hash, .. }
            | ServicesProvenance::LastGood { composed_hash, .. },
        ) => Some(composed_hash.clone()),
        _ => None,
    }
}

// Why: hashing the tree reads every file and the inventory reconciles once a
// minute, so the result is memoised and recomputed only when the tree's
// newest mtime or file count moves — a publish or a deploy, not a pass.
type Fingerprint = (SystemTime, usize);

fn base_tree_hash(root: &Path) -> Option<String> {
    static CACHE: OnceLock<Mutex<Option<(Fingerprint, String)>>> = OnceLock::new();
    let fingerprint = tree_fingerprint(root)?;
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock()
        && let Some((seen, hash)) = guard.as_ref()
        && *seen == fingerprint
    {
        return Some(hash.clone());
    }
    let files = match super::pack::collect_files(root, BUNDLE_ALLOWED_DIRS) {
        Ok(files) => files,
        Err(error) => {
            tracing::warn!(root = %root.display(), %error, "Base services tree unreadable; no base tree hash");
            return None;
        },
    };
    let hash = ServicesBundleManifest::compute_content_hash(&files);
    match cache.lock() {
        Ok(mut guard) => *guard = Some((fingerprint, hash.clone())),
        Err(error) => {
            tracing::warn!(%error, "Base tree hash memo poisoned; recomputing on every pass");
        },
    }
    Some(hash)
}

fn tree_fingerprint(root: &Path) -> Option<Fingerprint> {
    let mut newest = SystemTime::UNIX_EPOCH;
    let mut count = 0usize;
    for dir in BUNDLE_ALLOWED_DIRS {
        walk(&root.join(dir), &mut newest, &mut count);
    }
    (count > 0).then_some((newest, count))
}

fn walk(dir: &Path, newest: &mut SystemTime, count: &mut usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, newest, count);
        } else if let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) {
            *count += 1;
            if modified > *newest {
                *newest = modified;
            }
        }
    }
}
