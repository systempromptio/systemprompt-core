//! `GET /admin/services/status`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use systemprompt_config::ProfileBootstrap;
use systemprompt_loader::bundle::{BundleCache, cache_root};
use systemprompt_loader::services_root::{ActiveServicesRoot, ServicesRootBootstrap};
use systemprompt_models::services::bundle::ServicesBundleState;

use super::{ProvenanceView, ServicesStatusResponse, provenance_view, source_views};
use crate::error::ApiHttpError;

pub(super) async fn status() -> Result<Json<ServicesStatusResponse>, ApiHttpError> {
    let profile = ProfileBootstrap::get()
        .map_err(|e| ApiHttpError::internal_error(format!("profile not ready: {e}")))?;
    let state = BundleCache::new(cache_root(profile)).read_state();

    Ok(Json(build_status(
        ServicesRootBootstrap::get(),
        &profile.paths.services,
        !profile.services.sources.is_empty(),
        &state,
    )))
}

pub fn build_status(
    active: Option<&ActiveServicesRoot>,
    fallback_root: &str,
    has_sources: bool,
    state: &ServicesBundleState,
) -> ServicesStatusResponse {
    let active_root = active.map_or_else(
        || fallback_root.to_owned(),
        |root| root.path.display().to_string(),
    );

    let provenance = active.map_or_else(
        || ProvenanceView {
            kind: "bundled".to_owned(),
            composed_hash: None,
            error: None,
        },
        |root| provenance_view(&root.provenance),
    );

    ServicesStatusResponse {
        active_root,
        provenance,
        sources: if has_sources {
            source_views(state)
        } else {
            Vec::new()
        },
        last_reconciled_hash: state.last_reconciled_hash.clone(),
    }
}
