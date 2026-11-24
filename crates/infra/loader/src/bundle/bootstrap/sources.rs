//! Resolving every configured source, holding back the ones that fail.
//!
//! One source that cannot be fetched or verified must not keep the others
//! from advancing. Under `on_fetch_failure: use_last_good` a failing source
//! is resolved to the bundle already cached for it — provided this build can
//! still load that bundle — and the failure is returned as a
//! [`SourceRefusal`] beside the resolved members. A source with nothing
//! cached has no previous bundle to hold, and every other policy asks for a
//! whole-composition answer, so both propagate the error to the caller's
//! fallback exactly as before.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_manifest::profile::{FetchFailurePolicy, Profile, ServicesSource};

use super::fetch::{ResolvedSource, SourceContext, SourceFailure, resolve_source};
use crate::bundle::error::BundleResult;
use crate::bundle::refusal::{RefusalKind, SourceRefusal};
use crate::bundle::verify::require_core;

pub(super) struct Resolution {
    pub(super) resolved: Vec<ResolvedSource>,
    pub(super) refusals: Vec<SourceRefusal>,
}

pub(super) async fn resolve_sources(
    profile: &Profile,
    ctx: &SourceContext<'_>,
    resolve_secret: &(impl Fn(&str) -> Option<String> + Send + Sync),
) -> BundleResult<Resolution> {
    let holds_back = matches!(
        profile.services.on_fetch_failure,
        FetchFailurePolicy::UseLastGood
    );
    let mut resolution = Resolution {
        resolved: Vec::new(),
        refusals: Vec::new(),
    };
    for source in &profile.services.sources {
        let auth = source.auth_secret().and_then(resolve_secret);
        match resolve_source(source, ctx, auth).await {
            Ok(resolved) => resolution.resolved.push(resolved),
            Err(failure) => {
                let previous = if holds_back {
                    previous_bundle(source, ctx)
                } else {
                    None
                };
                let Some(previous) = previous else {
                    return Err(failure.error);
                };
                tracing::error!(
                    source = %source.name,
                    error = %failure.error,
                    serving = %previous.state.version,
                    "Source held back on its previous bundle"
                );
                resolution.refusals.push(refusal(&failure, &previous));
                resolution.resolved.push(previous);
            },
        }
    }
    Ok(resolution)
}

fn previous_bundle(source: &ServicesSource, ctx: &SourceContext<'_>) -> Option<ResolvedSource> {
    let known = ctx.state.sources.get(&source.name)?;
    if !ctx
        .cache
        .bundle_dir(&source.name, &known.content_hash)
        .is_dir()
    {
        return None;
    }
    let signed = ctx
        .cache
        .read_manifest(&source.name, &known.content_hash)
        .ok()?;
    require_core(&signed.manifest, ctx.core_version).ok()?;
    Some(ResolvedSource {
        name: source.name.clone(),
        content_hash: known.content_hash.clone(),
        signed,
        state: known.clone(),
    })
}

fn refusal(failure: &SourceFailure, previous: &ResolvedSource) -> SourceRefusal {
    SourceRefusal {
        source: previous.name.clone(),
        kind: RefusalKind::of(&failure.error),
        reason: failure.error.to_string(),
        attempted_digest: failure.attempted_digest.clone(),
        serving_version: previous.state.version.clone(),
        serving_digest: previous.state.digest.clone(),
        recorded_at: Utc::now(),
    }
}
