//! Admin endpoints reporting and refreshing the services bundle composition.
//!
//! `GET /status` answers "what tree is this instance actually serving, and
//! why" — including the failure text when the instance fell back to a cached
//! or baked tree, so an instance running yesterday's bundle does not look
//! healthy. `POST /refresh` re-runs the boot-time resolution against the
//! configured sources and reports whether the composition changed; the
//! running process keeps its old root either way, so `restart=true` asks the
//! supervisor to bring the process back on the new composition.
//!
//! Two refreshes must not fetch at once, so the router owns a single-flight
//! lock and the second caller is refused rather than queued behind a
//! multi-megabyte download.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod refresh;
mod status;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::{Extension, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_loader::services_root::{ActiveServicesRoot, ServicesProvenance};
use systemprompt_models::services::bundle::ServicesBundleState;
use systemprompt_runtime::AppContext;

pub use status::build_status;

pub(super) fn router() -> Router<AppContext> {
    Router::new()
        .route("/status", get(status::status))
        .route("/refresh", post(refresh::refresh))
        .layer(Extension(RefreshLock::default()))
}

/// Single-flight guard around the fetch-verify-compose pipeline.
///
/// Held for the whole refresh, so a caller that cannot take it is told the
/// refresh is already running instead of waiting on the lock and timing the
/// request out.
#[derive(Debug, Clone, Default)]
pub struct RefreshLock(Arc<tokio::sync::Mutex<()>>);

impl RefreshLock {
    pub fn try_acquire(&self) -> Option<tokio::sync::OwnedMutexGuard<()>> {
        Arc::clone(&self.0).try_lock_owned().ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceView {
    pub name: String,
    pub digest: String,
    pub version: String,
    pub content_hash: String,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceView {
    pub kind: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub composed_hash: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServicesStatusResponse {
    pub active_root: String,
    pub provenance: ProvenanceView,
    pub sources: Vec<SourceView>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_reconciled_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServicesRefreshResponse {
    pub changed: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub composed_hash: Option<String>,

    pub sources: Vec<SourceView>,
    pub restarting: bool,
}

pub fn source_views(state: &ServicesBundleState) -> Vec<SourceView> {
    state
        .sources
        .iter()
        .map(|(name, s)| SourceView {
            name: name.clone(),
            digest: s.digest.clone(),
            version: s.version.clone(),
            content_hash: s.content_hash.clone(),
            fetched_at: s.fetched_at,
        })
        .collect()
}

pub fn provenance_view(provenance: &ServicesProvenance) -> ProvenanceView {
    match provenance {
        ServicesProvenance::Bundled => ProvenanceView {
            kind: "bundled".to_owned(),
            composed_hash: None,
            error: None,
        },
        ServicesProvenance::Fetched { composed_hash, .. } => ProvenanceView {
            kind: "fetched".to_owned(),
            composed_hash: Some(composed_hash.clone()),
            error: None,
        },
        ServicesProvenance::LastGood {
            composed_hash,
            error,
        } => ProvenanceView {
            kind: "last_good".to_owned(),
            composed_hash: Some(composed_hash.clone()),
            error: Some(error.clone()),
        },
        ServicesProvenance::BundledFallback { error } => ProvenanceView {
            kind: "bundled".to_owned(),
            composed_hash: None,
            error: Some(error.clone()),
        },
    }
}

pub const fn composed_hash_of(active: &ActiveServicesRoot) -> Option<&str> {
    match &active.provenance {
        ServicesProvenance::Fetched { composed_hash, .. }
        | ServicesProvenance::LastGood { composed_hash, .. } => Some(composed_hash.as_str()),
        ServicesProvenance::Bundled | ServicesProvenance::BundledFallback { .. } => None,
    }
}
