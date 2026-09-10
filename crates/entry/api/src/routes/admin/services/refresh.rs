//! `POST /admin/services/refresh`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use axum::Json;
use axum::extract::{Extension, Query, State};
use serde::Deserialize;
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_loader::ServicesSourceBootstrap;
use systemprompt_loader::bundle::{BundleCache, cache_root};
use systemprompt_loader::services_root::ServicesRootBootstrap;
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_runtime::AppContext;

use super::{
    RefreshLock, ServicesRefreshResponse, composed_hash_of, provenance_view, source_views,
};
use crate::error::ApiHttpError;

const RESTART_DELAY: Duration = Duration::from_millis(250);
const RESTART_REASON: &str = "admin services refresh";

#[derive(Debug, Default, Deserialize)]
pub(super) struct RefreshQuery {
    #[serde(default)]
    restart: bool,
}

pub(super) async fn refresh(
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
    let secrets = SecretsBootstrap::get().ok();

    let resolved = ServicesSourceBootstrap::resolve(
        profile,
        |name| secrets.and_then(|s| s.get(name).cloned()),
        env!("CARGO_PKG_VERSION"),
    )
    .await?;

    let active_hash = ServicesRootBootstrap::get().and_then(composed_hash_of);
    let new_hash = composed_hash_of(&resolved);
    let changed = new_hash != active_hash;

    let state = BundleCache::new(cache_root(profile)).read_state();
    let restarting = changed && query.restart;

    tracing::info!(
        user_id = %req_ctx.user_id(),
        changed,
        composed_hash = new_hash.unwrap_or("none"),
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
        composed_hash: new_hash.map(str::to_owned),
        sources: source_views(&state),
        restarting,
    }))
}
