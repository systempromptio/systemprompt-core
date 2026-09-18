//! One catalogue resolution per user per sync.
//!
//! The disk catalogue with the managed overlay, the filtered candidate and
//! the plugin bundles, memoised in the application context's
//! [`MarketplaceCache`] and shared by the manifest route and every
//! plugin-file download that follows it.
//!
//! A hit costs one query (the managed stamp) and no catalogue work; a miss
//! does what the two routes used to do independently on every request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::UserId;
use systemprompt_marketplace::{
    AssembleRequest, ManifestService, MarketplaceError, NoopTrace, ResolvedCatalog, ResolvedKey,
};
use systemprompt_models::Profile;
use systemprompt_models::services::ServicesConfig;
use systemprompt_runtime::AppContext;

pub async fn resolve_for_user(
    ctx: &AppContext,
    services: &ServicesConfig,
    profile: &Profile,
    user_id: &UserId,
) -> Result<Arc<ResolvedCatalog>, MarketplaceError> {
    let cache = ctx.marketplace_cache();
    let services_root = ctx.app_paths().system().services();
    let (fingerprint, disk_catalog) = cache.catalog_with_fingerprint(
        services,
        services_root,
        &profile.server.api_external_url,
    )?;
    let owner = ctx.system_admin().id();
    let managed_stamp = ctx
        .managed_repository()
        .managed_catalog_stamp(owner, user_id)
        .await
        .map_err(MarketplaceError::Managed)?;
    let key = ResolvedKey {
        catalog: fingerprint,
        user: user_id.clone(),
        managed_stamp,
    };
    if let Some(hit) = cache.resolved(&key) {
        tracing::debug!(user_id = %user_id, "bridge: resolved catalogue served from memo");
        return Ok(hit);
    }

    let started = std::time::Instant::now();
    let catalog = (*disk_catalog)
        .clone()
        .with_organization_skills(ctx.managed_repository().as_ref().clone(), owner, user_id)
        .await?;
    let candidate = ManifestService::assemble_candidate_from_catalog(
        catalog.clone(),
        &AssembleRequest {
            services,
            services_root,
            filter: ctx.marketplace_filter().as_ref(),
            user_id,
            cache,
        },
        &mut NoopTrace,
    )
    .await?;
    let bundles = cache.bundles(services, &catalog.as_content())?;
    let resolved = Arc::new(ResolvedCatalog {
        catalog: Arc::new(catalog),
        candidate: Arc::new(candidate),
        bundles,
    });
    cache.store_resolved(key, Arc::clone(&resolved));
    tracing::info!(
        user_id = %user_id,
        elapsed_ms = started.elapsed().as_millis(),
        plugins = resolved.candidate.plugins.len(),
        skills = resolved.candidate.skills.len(),
        "bridge: resolved catalogue rebuilt"
    );
    Ok(resolved)
}
