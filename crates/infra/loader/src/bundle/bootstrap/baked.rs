//! The baked `services/` tree as the base member of a composition.
//!
//! A profile that pins only marketplace kits names no base: the base is the
//! tree shipped in the image at `paths.services`. It enters the composition
//! exactly like a fetched bundle — staged under the cache as `base` at its
//! content hash with a synthesised, unsigned `bundle.json` — so composition,
//! ownership checks and the authz reconcile see one uniform list of members
//! with the base first, and the same tree hash `/admin/sync` computes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use chrono::Utc;
use systemprompt_models::services::bundle::{
    BUNDLE_ALLOWED_DIRS, BUNDLE_MANIFEST_FILE, BundleSourceInfo, BundleSourceState,
    ServicesBundleManifest, SignedBundleManifest,
};

use super::fetch::ResolvedSource;
use crate::bundle::cache::{BundleCache, discard_staging};
use crate::bundle::compose::copy_tree;
use crate::bundle::error::{BundleError, BundleResult};
use crate::bundle::pack::build_manifest;

pub const BASE_SOURCE_NAME: &str = "base";
const BAKED_VERSION: &str = "baked";

#[must_use]
pub fn is_base(manifest: &ServicesBundleManifest) -> bool {
    manifest.owns.dirs.iter().any(|d| d == "config")
}

pub(super) fn stage_baked_base(
    cache: &BundleCache,
    services_root: &Path,
    core_version: &str,
) -> BundleResult<ResolvedSource> {
    if !services_root.join("config/config.yaml").is_file() {
        return Err(BundleError::policy(format!(
            "no pinned source is a base and no baked services tree at {}",
            services_root.display()
        )));
    }
    let manifest = build_manifest(
        services_root,
        BAKED_VERSION,
        &format!(">={core_version}"),
        BundleSourceInfo::default(),
    )?;
    let content_hash = manifest.content_hash.clone();
    let signed = SignedBundleManifest {
        manifest,
        signature: None,
    };

    let target = cache.bundle_dir(BASE_SOURCE_NAME, &content_hash);
    if !target.is_dir() {
        cache.prepare()?;
        let staging = cache.bundle_dir(
            BASE_SOURCE_NAME,
            &format!("{content_hash}.tmp-{}", std::process::id()),
        );
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir_all(&staging)?;
        for dir in BUNDLE_ALLOWED_DIRS {
            let from = services_root.join(dir);
            if from.is_dir() {
                let to = staging.join(dir);
                fs::create_dir_all(&to)?;
                copy_tree(&from, &to, false)?;
            }
        }
        let raw = serde_json::to_vec_pretty(&signed)
            .map_err(|e| BundleError::policy(format!("baked base manifest: {e}")))?;
        fs::write(staging.join(BUNDLE_MANIFEST_FILE), raw)?;
        if let Err(e) = fs::rename(&staging, &target) {
            discard_staging(&staging);
            if !target.is_dir() {
                return Err(BundleError::extract(&target, e));
            }
        }
    }

    Ok(ResolvedSource {
        name: BASE_SOURCE_NAME.to_owned(),
        content_hash: content_hash.clone(),
        signed,
        state: BundleSourceState {
            digest: format!("tree:{content_hash}"),
            version: BAKED_VERSION.to_owned(),
            content_hash,
            fetched_at: Utc::now(),
        },
    })
}
