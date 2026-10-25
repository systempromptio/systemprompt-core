//! Boot-time resolution of the services root from configured bundle sources.
//!
//! The happy path is fetch, verify, compose, load-validate, swap, record. Every
//! other path is a named fallback carrying the error that caused it: an
//! instance serving last-good content reports [`ServicesProvenance::LastGood`]
//! with the failure text, so "we are running yesterday's bundle" is visible
//! rather than inferred from a log line that scrolled past. `fail_closed`
//! refuses to boot instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod baked;
mod fetch;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use systemprompt_manifest::profile::{FetchFailurePolicy, Profile};
use systemprompt_manifest::services::bundle::ServicesBundleState;

use super::cache::BundleCache;
use super::compose::{BundleMember, compose};
use super::degraded::DegradedRecord;
use super::error::{BundleError, BundleResult};
use crate::services_root::{ActiveServicesRoot, ServicesProvenance, ServicesRootBootstrap};

use baked::{is_base, stage_baked_base};
use fetch::{ResolvedSource, SourceContext, resolve_source};

const HTTP_TIMEOUT: Duration = Duration::from_secs(60);
const CONFIG_RELPATH: &str = "config/config.yaml";

#[derive(Debug, Clone, Copy)]
pub struct ServicesSourceBootstrap;

impl ServicesSourceBootstrap {
    pub async fn try_run(
        profile: &Profile,
        resolve_secret: impl Fn(&str) -> Option<String> + Send + Sync,
        core_version: &str,
    ) -> BundleResult<&'static ActiveServicesRoot> {
        if let Some(active) = ServicesRootBootstrap::get() {
            return Ok(active);
        }
        Self::resolve(profile, resolve_secret, core_version)
            .await
            .map(ServicesRootBootstrap::install)
    }

    pub async fn resolve(
        profile: &Profile,
        resolve_secret: impl Fn(&str) -> Option<String> + Send + Sync,
        core_version: &str,
    ) -> BundleResult<ActiveServicesRoot> {
        if profile.services.sources.is_empty() {
            let path = PathBuf::from(&profile.paths.services);
            return Ok(ActiveServicesRoot {
                base: path.clone(),
                path,
                provenance: ServicesProvenance::Bundled,
            });
        }

        let cache = BundleCache::new(cache_root(profile));
        match Self::compose_sources(profile, &cache, &resolve_secret, core_version).await {
            Ok(active) => {
                if let Err(error) = cache.clear_degraded() {
                    tracing::warn!(error = %error, "Could not clear the degraded services record");
                }
                Ok(active)
            },
            Err(e) => {
                tracing::error!(error = %e, "Services bundle refresh failed");
                let active = Self::fall_back(profile, &cache, e)?;
                if let Some(record) = DegradedRecord::for_provenance(&active.provenance)
                    && let Err(error) = cache.write_degraded(&record)
                {
                    tracing::warn!(error = %error, "Could not record the degraded services source");
                }
                Ok(active)
            },
        }
    }

    async fn compose_sources(
        profile: &Profile,
        cache: &BundleCache,
        resolve_secret: &(impl Fn(&str) -> Option<String> + Send + Sync),
        core_version: &str,
    ) -> BundleResult<ActiveServicesRoot> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| BundleError::policy_context("http client", e))?;

        let previous = cache.read_state();
        let ctx = SourceContext {
            cache,
            state: &previous,
            core_version,
            client: &client,
        };
        let mut resolved: Vec<ResolvedSource> = Vec::new();
        for source in &profile.services.sources {
            let auth = source.auth_secret().and_then(resolve_secret);
            resolved.push(resolve_source(source, &ctx, auth).await?);
        }
        // Why: a profile that pins only kits names no base; the tree baked
        // into the image is the base, and it must be member zero so ownership
        // and the authz reconcile treat every pinned source as a kit.
        if !resolved
            .first()
            .is_some_and(|r| is_base(&r.signed.manifest))
        {
            let base = stage_baked_base(cache, Path::new(&profile.paths.services), core_version)?;
            resolved.insert(0, base);
        }

        let members: Vec<BundleMember<'_>> = resolved
            .iter()
            .map(|r| BundleMember {
                name: r.name.clone(),
                content_hash: r.content_hash.clone(),
                manifest: &r.signed.manifest,
            })
            .collect();
        let (composed_path, composed_hash) = compose(cache, &members)?;
        let kits: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();
        let retained = previous.composed_hash == composed_hash;
        refuse_unloadable(&composed_path, &kits, retained)?;
        cache.swap_current(&composed_path)?;

        let state = ServicesBundleState {
            composed_hash: composed_hash.clone(),
            last_reconciled_hash: previous.last_reconciled_hash.clone(),
            sources: resolved
                .iter()
                .map(|r| (r.name.clone(), r.state.clone()))
                .collect(),
        };
        cache.write_state(&state)?;
        cache.gc(&composed_hash)?;

        let versions: BTreeMap<String, String> = resolved
            .iter()
            .map(|r| (r.name.clone(), r.signed.manifest.version.clone()))
            .collect();
        tracing::info!(composed_hash = %composed_hash, sources = resolved.len(), "Services bundles composed");

        Ok(ActiveServicesRoot {
            path: cache.current_link(),
            base: PathBuf::from(&profile.paths.services),
            provenance: ServicesProvenance::Fetched {
                composed_hash,
                versions,
            },
        })
    }

    fn fall_back(
        profile: &Profile,
        cache: &BundleCache,
        error: BundleError,
    ) -> BundleResult<ActiveServicesRoot> {
        match profile.services.on_fetch_failure {
            FetchFailurePolicy::FailClosed => Err(BundleError::policy_context(
                "services.on_fetch_failure is fail_closed",
                error,
            )),
            FetchFailurePolicy::UseLastGood => last_good(profile, cache, &error)
                .map_or_else(|| bundled_fallback(profile, error), Ok),
            FetchFailurePolicy::UseBundled => bundled_fallback(profile, error),
        }
    }
}

fn refuse_unloadable(composed: &Path, kits: &[&str], retained: bool) -> BundleResult<()> {
    let Err(error) = crate::ConfigLoader::load_from_path(&composed.join(CONFIG_RELPATH)) else {
        return Ok(());
    };
    if !retained && let Err(remove) = std::fs::remove_dir_all(composed) {
        tracing::warn!(error = %remove, path = %composed.display(), "Could not remove the rejected composition");
    }
    Err(BundleError::policy_context(
        format!(
            "the composed services tree (sources: {}) does not load, so it was not swapped in",
            kits.join(", ")
        ),
        error,
    ))
}

fn last_good(
    profile: &Profile,
    cache: &BundleCache,
    error: &BundleError,
) -> Option<ActiveServicesRoot> {
    let current = cache.current_root()?;
    let state = cache.read_state();
    if state.composed_hash.is_empty() {
        return None;
    }
    tracing::error!(
        composed_hash = %state.composed_hash,
        error = %error,
        "Serving the last-good services composition after a failed refresh"
    );
    Some(ActiveServicesRoot {
        path: current,
        base: PathBuf::from(&profile.paths.services),
        provenance: ServicesProvenance::LastGood {
            composed_hash: state.composed_hash,
            error: error.to_string(),
        },
    })
}

fn bundled_fallback(profile: &Profile, error: BundleError) -> BundleResult<ActiveServicesRoot> {
    let root = PathBuf::from(&profile.paths.services);
    if !root.join("config/config.yaml").is_file() {
        return Err(BundleError::policy_context(
            format!(
                "no cached bundle and no baked services tree at {}",
                root.display()
            ),
            error,
        ));
    }
    tracing::error!(
        path = %root.display(),
        error = %error,
        "Serving the services tree baked into the image after a failed refresh"
    );
    Ok(ActiveServicesRoot {
        base: root.clone(),
        path: root,
        provenance: ServicesProvenance::BundledFallback {
            error: error.to_string(),
        },
    })
}

#[must_use]
pub fn cache_root(profile: &Profile) -> PathBuf {
    profile.services.cache_dir.as_ref().map_or_else(
        || PathBuf::from(&profile.paths.system).join("services-cache"),
        PathBuf::from,
    )
}
