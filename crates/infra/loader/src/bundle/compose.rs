//! Overlaying several verified bundles into one services root.
//!
//! Composition is by-id, not last-write-wins: two bundles claiming the same
//! marketplace, plugin, skill, rule, hook or artifact is a boot error naming
//! both sources, because silently preferring one would make which access
//! rules an instance enforces depend on profile ordering. Marketplace
//! directories are shared by construction — their ids are disjoint — while a
//! base-only directory such as `access-control/` may have exactly one owner.
//!
//! The composed root is content-addressed by the ordered list of member
//! hashes, so recomposing an unchanged set is a directory-exists check.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use systemprompt_models::services::bundle::{
    BUNDLE_MANIFEST_FILE, MARKETPLACE_BUNDLE_DIRS, ServicesBundleManifest,
};

use super::cache::BundleCache;
use super::error::{BundleError, BundleResult};
use super::verify::require_marketplace_only;

#[derive(Debug)]
pub struct BundleMember<'a> {
    pub name: String,
    pub content_hash: String,
    pub manifest: &'a ServicesBundleManifest,
}

#[must_use]
pub fn composed_hash(members: &[BundleMember<'_>]) -> String {
    let mut hasher = Sha256::new();
    for member in members {
        hasher.update(format!("{}\0{}\n", member.name, member.content_hash).as_bytes());
    }
    hex::encode(hasher.finalize())
}

pub fn compose(
    cache: &BundleCache,
    members: &[BundleMember<'_>],
) -> BundleResult<(PathBuf, String)> {
    let hash = composed_hash(members);
    let target = cache.composed_dir(&hash);
    if target.is_dir() {
        return Ok((target, hash));
    }

    check_ownership(members)?;

    cache.prepare()?;
    let staging = cache.composed_dir(&format!("{hash}.tmp-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;

    for member in members {
        let source = cache.bundle_dir(&member.name, &member.content_hash);
        copy_tree(&source, &staging, true)?;
    }

    match fs::rename(&staging, &target) {
        Ok(()) => Ok((target, hash)),
        Err(e) if target.is_dir() => {
            drop(fs::remove_dir_all(&staging));
            tracing::debug!(error = %e, hash = %hash, "Composed root already published");
            Ok((target, hash))
        },
        Err(e) => {
            drop(fs::remove_dir_all(&staging));
            Err(BundleError::extract(&target, e))
        },
    }
}

fn check_ownership(members: &[BundleMember<'_>]) -> BundleResult<()> {
    for member in members.iter().skip(1) {
        require_marketplace_only(member.manifest)?;
    }

    let mut owners: HashMap<(&str, String), &str> = HashMap::new();
    for member in members {
        let owns = &member.manifest.owns;
        let categories: [(&str, &Vec<String>); 6] = [
            ("marketplace", &owns.marketplaces),
            ("plugin", &owns.plugins),
            ("skill", &owns.skills),
            ("rule", &owns.rules),
            ("hook", &owns.hooks),
            ("artifact", &owns.artifacts),
        ];
        for (kind, ids) in categories {
            for id in ids {
                claim(&mut owners, kind, id.clone(), &member.name)?;
            }
        }
        for dir in &owns.dirs {
            if MARKETPLACE_BUNDLE_DIRS.contains(&dir.as_str()) {
                continue;
            }
            claim(&mut owners, "directory", dir.clone(), &member.name)?;
        }
    }
    Ok(())
}

fn claim<'a>(
    owners: &mut HashMap<(&'a str, String), &'a str>,
    kind: &'a str,
    id: String,
    name: &'a str,
) -> BundleResult<()> {
    if let Some(first) = owners.insert((kind, id.clone()), name) {
        return Err(BundleError::Ownership {
            id,
            kind: kind.to_owned(),
            first: first.to_owned(),
            second: name.to_owned(),
        });
    }
    Ok(())
}

fn copy_tree(source: &Path, dest: &Path, skip_manifest: bool) -> BundleResult<()> {
    for entry in fs::read_dir(source)? {
        let path = entry?.path();
        let Some(name) = path.file_name() else {
            continue;
        };
        if skip_manifest && path.is_file() && name == BUNDLE_MANIFEST_FILE {
            continue;
        }
        let target = dest.join(name);
        if path.is_dir() {
            fs::create_dir_all(&target)?;
            copy_tree(&path, &target, false)?;
        } else if path.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            if fs::hard_link(&path, &target).is_err() {
                fs::copy(&path, &target)?;
            }
        }
    }
    Ok(())
}
