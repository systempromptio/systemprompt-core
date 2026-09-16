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
use systemprompt_loader::bundle::bootstrap::baked::BASE_SOURCE_NAME;
use systemprompt_loader::bundle::{BundleCache, cache_root};
use systemprompt_loader::services_root::ServicesRootBootstrap;
use systemprompt_loader::{ConfigLoader, ServicesSourceBootstrap};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_models::services::bundle::ServicesBundleState;
use systemprompt_runtime::AppContext;
use systemprompt_runtime::managed::inventory::publish_latest;
use systemprompt_runtime::services_reconcile::{ReconcileOutcome, reconcile_fetched_services};

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

    // Why: the boot-time root is a static; after an in-place import the cache
    // state names the composition actually being served, so "changed" is
    // measured against that and a repeat import is a no-op.
    let cache = BundleCache::new(cache_root(profile));
    let previous = cache.read_state();
    let served_hash = (!previous.composed_hash.is_empty())
        .then_some(previous.composed_hash.clone())
        .or_else(|| {
            ServicesRootBootstrap::get()
                .and_then(composed_hash_of)
                .map(str::to_owned)
        });

    let resolved = ServicesSourceBootstrap::resolve(
        profile,
        |name| secrets.get(name).cloned(),
        env!("CARGO_PKG_VERSION"),
    )
    .await?;

    let new_hash = composed_hash_of(&resolved).map(str::to_owned);
    let changed = new_hash != served_hash;
    // Why: a composition that was swapped in but never projected (a failed
    // earlier reconcile) is finished by the next import even though nothing
    // else changed.
    let unreconciled = new_hash.is_some() && previous.last_reconciled_hash != new_hash;
    let mut reconciled = false;
    if changed || unreconciled {
        // Why: the boot-time root is a static that still names the previous
        // tree; the recomposed tree is the one whose config is projected.
        let services =
            ConfigLoader::reload_from_path(&resolved.path.join("config").join("config.yaml"))
                .map_err(|e| {
                    ApiHttpError::internal_error(format!("recomposed services config: {e}"))
                })?;
        let outcome = reconcile_fetched_services(profile, &resolved, &services, ctx.db_pool())
            .await
            .map_err(|e| ApiHttpError::internal_error(format!("services reconcile: {e}")))?;
        reconciled = outcome == ReconcileOutcome::Projected;

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
// serve, so the caller is told a restart would complete it. A manifest that
// cannot be read may own hooks, so it recommends the restart too.
fn owns_static_config(cache: &BundleCache, state: &ServicesBundleState) -> bool {
    state
        .sources
        .iter()
        .filter(|(name, _)| name.as_str() != BASE_SOURCE_NAME)
        .any(|(name, fetched)| match cache.read_manifest(name, &fetched.content_hash) {
            Ok(signed) => !signed.manifest.owns.hooks.is_empty(),
            Err(error) => {
                tracing::warn!(source = %name, %error, "Cached bundle manifest unreadable; recommending a restart");
                true
            },
        })
}
