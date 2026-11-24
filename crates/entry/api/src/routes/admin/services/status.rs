//! `GET /admin/services/status`.
//!
//! The root a process booted on is fixed for its lifetime, but an in-place
//! import can fall back, or recover, after boot. The bundle cache records
//! both, so the provenance reported here is the cache's when it disagrees
//! with the boot root.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::State;
use systemprompt_loader::bundle::{BundleCache, DegradedRecord, cache_root};
use systemprompt_manifest::services::bundle::ServicesBundleState;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_runtime::AppContext;

use super::{ProvenanceView, ServicesStatusResponse, provenance_view, source_views};
use crate::kernel::ApiHttpError;

pub(super) async fn status(
    State(ctx): State<AppContext>,
) -> Result<Json<ServicesStatusResponse>, ApiHttpError> {
    let profile = ctx.profile();
    let cache = BundleCache::new(cache_root(profile));
    let state = cache.read_state();
    let has_sources = !profile.services.sources.is_empty();

    let mut body = build_status(
        ctx.boot().services_root(),
        &profile.paths.services,
        has_sources,
        &state,
    );
    if has_sources {
        body.provenance =
            served_provenance(body.provenance, cache.read_degraded().as_ref(), &state);
        body.refused = cache.read_refusals()?;
    }
    Ok(Json(body))
}

// JSON: `/readyz` and `/health` carry this block as an open JSON object.
pub fn held_back_block(has_sources: bool, cache: &BundleCache) -> Option<serde_json::Value> {
    if !has_sources {
        return None;
    }
    match cache.read_refusals() {
        Ok(refusals) if refusals.is_empty() => None,
        Ok(refusals) => Some(serde_json::json!({
            "serving": "held_back",
            "sources": refusals,
        })),
        Err(error) => {
            tracing::warn!(error = %error, "Held-back services sources could not be read");
            Some(serde_json::json!({
                "serving": "unknown",
                "error": "the record of held-back sources cannot be read",
            }))
        },
    }
}

pub fn served_provenance(
    boot: ProvenanceView,
    degraded: Option<&DegradedRecord>,
    state: &ServicesBundleState,
) -> ProvenanceView {
    let composed_hash = (!state.composed_hash.is_empty()).then(|| state.composed_hash.clone());
    match degraded {
        Some(record) => ProvenanceView {
            kind: record.serving.clone(),
            composed_hash: composed_hash.filter(|_| record.serving == "last_good"),
            error: Some(record.error.clone()),
        },
        None if boot.error.is_some() && composed_hash.is_some() => ProvenanceView {
            kind: "fetched".to_owned(),
            composed_hash,
            error: None,
        },
        None => boot,
    }
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
        refused: Vec::new(),
        last_reconciled_hash: state.last_reconciled_hash.clone(),
    }
}
