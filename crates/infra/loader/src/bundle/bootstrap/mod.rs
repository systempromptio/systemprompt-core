//! Boot-time resolution of the services root from configured bundle sources.
//!
//! The happy path is fetch, verify, compose, swap, record. Every other path
//! is a named fallback carrying the error that caused it: an instance serving
//! last-good content reports [`ServicesProvenance::LastGood`] with the failure
//! text, so "we are running yesterday's bundle" is visible rather than
//! inferred from a log line that scrolled past. `fail_closed` refuses to boot
//! instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod fetch;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use systemprompt_models::profile::{FetchFailurePolicy, Profile};
use systemprompt_models::services::bundle::ServicesBundleState;

use super::cache::BundleCache;
use super::compose::{BundleMember, compose};
use super::error::{BundleError, BundleResult};
use crate::services_root::{ActiveServicesRoot, ServicesProvenance, ServicesRootBootstrap};

use fetch::{ResolvedSource, SourceContext, resolve_source};

const HTTP_TIMEOUT: Duration = Duration::from_secs(60);

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
            return Ok(ActiveServicesRoot {
                path: PathBuf::from(&profile.paths.services),
                provenance: ServicesProvenance::Bundled,
            });
        }

        let cache = BundleCache::new(cache_root(profile));
        match Self::compose_sources(profile, &cache, &resolve_secret, core_version).await {
            Ok(active) => Ok(active),
            Err(e) => {
                tracing::error!(error = %e, "Services bundle refresh failed");
                Self::fall_back(profile, &cache, &e)
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
            .map_err(|e| BundleError::policy(format!("http client: {e}")))?;

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

        let members: Vec<BundleMember<'_>> = resolved
            .iter()
            .map(|r| BundleMember {
                name: r.name.clone(),
                content_hash: r.content_hash.clone(),
                manifest: &r.signed.manifest,
            })
            .collect();
        let (composed_path, composed_hash) = compose(cache, &members)?;
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
            provenance: ServicesProvenance::Fetched {
                composed_hash,
                versions,
            },
        })
    }

    fn fall_back(
        profile: &Profile,
        cache: &BundleCache,
        error: &BundleError,
    ) -> BundleResult<ActiveServicesRoot> {
        match profile.services.on_fetch_failure {
            FetchFailurePolicy::FailClosed => Err(BundleError::policy(format!(
                "services.on_fetch_failure is fail_closed: {error}"
            ))),
            FetchFailurePolicy::UseLastGood => {
                last_good(cache, error).map_or_else(|| bundled_fallback(profile, error), Ok)
            },
            FetchFailurePolicy::UseBundled => bundled_fallback(profile, error),
        }
    }
}

fn last_good(cache: &BundleCache, error: &BundleError) -> Option<ActiveServicesRoot> {
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
        provenance: ServicesProvenance::LastGood {
            composed_hash: state.composed_hash,
            error: error.to_string(),
        },
    })
}

fn bundled_fallback(profile: &Profile, error: &BundleError) -> BundleResult<ActiveServicesRoot> {
    let root = PathBuf::from(&profile.paths.services);
    if !root.join("config/config.yaml").is_file() {
        return Err(BundleError::policy(format!(
            "no cached bundle and no baked services tree at {}: {error}",
            root.display()
        )));
    }
    tracing::error!(
        path = %root.display(),
        error = %error,
        "Serving the services tree baked into the image after a failed refresh"
    );
    Ok(ActiveServicesRoot {
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
