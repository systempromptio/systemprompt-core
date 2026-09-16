//! `POST /admin/services/refresh`.
//!
//! Re-resolves the configured sources, recomposes the tree and — when the
//! composition changed — projects the new composition into the authz tables
//! and refreshes the skill inventory, all in-process. The routes that hand
//! marketplaces, plugins and skills to clients reload the services tree per
//! request through the `current` link the recompose just swapped, so a
//! marketplace-only kit is live the moment this returns. `restart=true`
//! remains an explicit opt-in for the one thing a running process cannot
//! re-read: the static services config behind governance hooks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use axum::Json;
use axum::extract::{Extension, Query, State};
use serde::Deserialize;
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_loader::bundle::{BundleCache, cache_root};
use systemprompt_loader::services_root::ServicesRootBootstrap;
use systemprompt_loader::{ConfigLoader, ServicesSourceBootstrap};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_models::services::bundle::ServicesBundleState;
use systemprompt_runtime::AppContext;
use systemprompt_runtime::optimization::inventory::publish_latest;
use systemprompt_runtime::services_reconcile::reconcile_fetched_services;

use super::{
    RefreshLock, ServicesRefreshResponse, composed_hash_of, provenance_view, source_views,
};
use crate::error::ApiHttpError;

const RESTART_DELAY: Duration = Duration::from_millis(250);
const RESTART_REASON: &str = "admin services refresh";

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct RefreshQuery {
    #[serde(default)]
    pub restart: bool,
}

pub async fn refresh(
    State(ctx): State<AppContext>,
    Extension(lock): Extension<RefreshLock>,
    Extension(req_ctx): Extension<RequestContext>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<ServicesRefreshResponse>, ApiHttpError> {
    let Some(_guard) = lock.try_acquire() else {
        return Err(ApiError::conflict("a services refresh is already running").into());
    };

    let profile = ProfileBootstrap::get()
        .map_err(|e| ApiHttpError::internal_error(format!("profile not ready: {e}")))?;
    let secrets = SecretsBootstrap::get()
        .map_err(|e| ApiHttpError::internal_error(format!("secrets not ready: {e}")))?;

    let resolved = ServicesSourceBootstrap::resolve(
        profile,
        |name| secrets.get(name).cloned(),
        env!("CARGO_PKG_VERSION"),
    )
    .await?;

    let active_hash = ServicesRootBootstrap::get().and_then(composed_hash_of);
    let new_hash = composed_hash_of(&resolved).map(str::to_owned);
    let changed = new_hash.as_deref() != active_hash;

    let cache = BundleCache::new(cache_root(profile));
    let mut reconciled = false;
    if changed {
        let services = ConfigLoader::load().map_err(|e| {
            ApiHttpError::internal_error(format!("recomposed services config: {e}"))
        })?;
        reconcile_fetched_services(profile, &resolved, &services, ctx.db_pool())
            .await
            .map_err(|e| ApiHttpError::internal_error(format!("services reconcile: {e}")))?;
        reconciled = true;

        let system_admin = ctx.system_admin().id().clone();
        if let Err(error) = publish_latest(&ctx, &system_admin, req_ctx.user_id()).await {
            tracing::warn!(%error, "Inventory refresh after services import failed; the scheduled pass will retry");
        }
    }

    let state = cache.read_state();
    let restart_recommended = changed && owns_static_config(&cache, &state);
    let restarting = changed && query.restart;

    tracing::info!(
        user_id = %req_ctx.user_id(),
        changed,
        reconciled,
        restart_recommended,
        composed_hash = new_hash.as_deref().unwrap_or("none"),
        provenance = %provenance_view(&resolved.provenance).kind,
        restarting,
        "Admin services refresh"
    );

    if restarting {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(RESTART_DELAY).await;
            ctx.request_restart(RESTART_REASON);
        });
    }

    Ok(Json(ServicesRefreshResponse {
        changed,
        composed_hash: new_hash,
        sources: source_views(&state),
        reconciled,
        restart_recommended,
        restarting,
    }))
}

// Why: governance hooks are read once at boot into the static services config;
// a bundle that ships hooks is the one case an in-process import cannot fully
// serve, so the caller is told a restart would complete it.
fn owns_static_config(cache: &BundleCache, state: &ServicesBundleState) -> bool {
    state.sources.iter().skip(1).any(|(name, fetched)| {
        cache
            .read_manifest(name, &fetched.content_hash)
            .is_ok_and(|signed| !signed.manifest.owns.hooks.is_empty())
    })
}
