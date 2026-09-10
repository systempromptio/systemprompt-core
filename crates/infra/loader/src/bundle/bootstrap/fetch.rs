//! Fetch, verify and cache one configured bundle source.
//!
//! The extracted tree is verified in a staging directory and only then
//! renamed into its content-addressed home, so a cache entry that exists is a
//! cache entry that passed every check. A partially-written staging directory
//! is removed on the way out rather than left to be mistaken for a good one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;

use chrono::Utc;
use systemprompt_models::profile::ServicesSource;
use systemprompt_models::services::bundle::{
    BUNDLE_ALLOWED_DIRS, BundleSourceState, ServicesBundleState, SignedBundleManifest,
};

use crate::bundle::cache::BundleCache;
use crate::bundle::error::{BundleError, BundleResult};
use crate::bundle::extract::{ExtractOptions, TarLayout, extract_tarball};
use crate::bundle::source::{AnyFetcher, BundleFetcher, MAX_BUNDLE_BYTES};
use crate::bundle::verify::{verify_bundle, verify_extracted};

pub(super) struct ResolvedSource {
    pub(super) name: String,
    pub(super) content_hash: String,
    pub(super) signed: SignedBundleManifest,
    pub(super) state: BundleSourceState,
}

pub(super) struct SourceContext<'a> {
    pub(super) cache: &'a BundleCache,
    pub(super) state: &'a ServicesBundleState,
    pub(super) core_version: &'a str,
    pub(super) client: &'a reqwest::Client,
}

pub(super) async fn resolve_source(
    source: &ServicesSource,
    ctx: &SourceContext<'_>,
    auth: Option<String>,
) -> BundleResult<ResolvedSource> {
    let (cache, state) = (ctx.cache, ctx.state);
    let fetcher = AnyFetcher::from_source(source, auth, ctx.client)?;
    let remote = fetcher.head().await?;

    if !remote.is_unknown()
        && let Some(known) = state.sources.get(&source.name)
        && known.digest == remote.digest
        && cache.bundle_dir(&source.name, &known.content_hash).is_dir()
    {
        let signed = cache.read_manifest(&source.name, &known.content_hash)?;
        tracing::debug!(source = %source.name, digest = %known.digest, "Reusing cached bundle");
        return Ok(ResolvedSource {
            name: source.name.clone(),
            content_hash: known.content_hash.clone(),
            signed,
            state: known.clone(),
        });
    }

    cache.prepare()?;
    let staging_dir = cache.root().join(format!("tmp-{}", std::process::id()));
    fs::create_dir_all(&staging_dir)?;
    let archive = staging_dir.join(format!("{}.tar.gz", source.name));

    let outcome = download_and_install(source, ctx, &fetcher, &archive).await;
    drop(fs::remove_dir_all(&staging_dir));
    let (signed, digest) = outcome?;

    let content_hash = signed.manifest.content_hash.clone();
    let state = BundleSourceState {
        digest: if remote.is_unknown() {
            digest
        } else {
            remote.digest
        },
        version: signed.manifest.version.clone(),
        content_hash: content_hash.clone(),
        fetched_at: Utc::now(),
    };
    Ok(ResolvedSource {
        name: source.name.clone(),
        content_hash,
        signed,
        state,
    })
}

async fn download_and_install(
    source: &ServicesSource,
    ctx: &SourceContext<'_>,
    fetcher: &AnyFetcher,
    archive: &std::path::Path,
) -> BundleResult<(SignedBundleManifest, String)> {
    let downloaded = fetcher.fetch(archive).await?;
    let verification = source
        .verification()
        .ok_or_else(|| BundleError::policy(format!("source {} has no transport", source.name)))?;
    let signed = verify_bundle(archive, verification, ctx.core_version)?;

    let staged = archive.with_extension("tree");
    if staged.exists() {
        fs::remove_dir_all(&staged)?;
    }
    fs::create_dir_all(&staged)?;
    extract_tarball(
        archive,
        &staged,
        &ExtractOptions {
            allowed_dirs: BUNDLE_ALLOWED_DIRS,
            max_bytes: MAX_BUNDLE_BYTES,
            layout: TarLayout::Bundle,
        },
    )?;
    verify_extracted(&staged, &signed.manifest)?;

    let target = ctx
        .cache
        .bundle_dir(&source.name, &signed.manifest.content_hash);
    if target.is_dir() {
        fs::remove_dir_all(&target)?;
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&staged, &target).map_err(|e| BundleError::extract(&target, e))?;

    tracing::info!(
        source = %source.name,
        version = %signed.manifest.version,
        content_hash = %signed.manifest.content_hash,
        "Installed services bundle"
    );
    Ok((signed, downloaded.digest))
}
