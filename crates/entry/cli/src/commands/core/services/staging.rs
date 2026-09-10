//! Staging trees and archives into a throwaway bundle cache so that
//! `services validate` composes them exactly the way an instance does.
//!
//! A working tree has no manifest, so it is packed into a temporary archive
//! and extracted back out rather than copied: the composed root then carries
//! the same file set, checksums and ownership the published bundle would, and
//! a cross-bundle collision surfaces here instead of at the next boot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use systemprompt_loader::bundle::source::MAX_BUNDLE_BYTES;
use systemprompt_loader::bundle::{
    BundleCache, ExtractOptions, TarLayout, compose, extract_tarball, pack, verify,
};
use systemprompt_models::services::bundle::{
    BUNDLE_ALLOWED_DIRS, BundleSourceInfo, ServicesBundleManifest, SignedBundleManifest,
};

pub const STAGING_VERSION: &str = "0.0.0-validate";
pub const STAGING_REQUIRES_CORE: &str = ">=0.0";

#[derive(Debug)]
pub struct StagedBundle {
    pub name: String,
    pub root: PathBuf,
    pub manifest: ServicesBundleManifest,
}

pub fn stage_tree(
    cache: &BundleCache,
    name: &str,
    tree: &Path,
    scratch: &Path,
) -> Result<StagedBundle> {
    let manifest = pack::build_manifest(
        tree,
        STAGING_VERSION,
        STAGING_REQUIRES_CORE,
        BundleSourceInfo::default(),
    )
    .with_context(|| format!("Failed to read services tree {}", tree.display()))?;

    let archive = scratch.join(format!("{name}.tar.gz"));
    let signed = SignedBundleManifest {
        manifest,
        signature: None,
    };
    pack::write_tarball(tree, &signed, &archive)
        .with_context(|| format!("Failed to stage {}", tree.display()))?;

    let root = extract_into(cache, name, &signed.manifest, &archive)?;
    Ok(StagedBundle {
        name: name.to_owned(),
        root,
        manifest: signed.manifest,
    })
}

pub fn stage_archive(cache: &BundleCache, name: &str, archive: &Path) -> Result<StagedBundle> {
    let signed = verify::read_manifest(archive)
        .with_context(|| format!("Failed to read {}", archive.display()))?;
    let root = extract_into(cache, name, &signed.manifest, archive)?;
    Ok(StagedBundle {
        name: name.to_owned(),
        root,
        manifest: signed.manifest,
    })
}

fn extract_into(
    cache: &BundleCache,
    name: &str,
    manifest: &ServicesBundleManifest,
    archive: &Path,
) -> Result<PathBuf> {
    let dest = cache.bundle_dir(name, &manifest.content_hash);
    if dest.is_dir() {
        return Ok(dest);
    }
    extract_tarball(
        archive,
        &dest,
        &ExtractOptions {
            allowed_dirs: BUNDLE_ALLOWED_DIRS,
            max_bytes: MAX_BUNDLE_BYTES,
            layout: TarLayout::Bundle,
        },
    )
    .with_context(|| format!("Failed to extract {}", archive.display()))?;
    Ok(dest)
}

pub fn compose_staged(cache: &BundleCache, members: &[StagedBundle]) -> Result<PathBuf> {
    let refs: Vec<compose::BundleMember<'_>> = members
        .iter()
        .map(|staged| compose::BundleMember {
            name: staged.name.clone(),
            content_hash: staged.manifest.content_hash.clone(),
            manifest: &staged.manifest,
        })
        .collect();
    let (root, _hash) = compose(cache, &refs).context("Bundles could not be composed")?;
    Ok(root)
}
