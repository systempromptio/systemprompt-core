//! The Drive's browsing listing under `/api/v1/documents` and
//! `/api/v1/document-folders`, read from the same in-memory store the sync
//! feed serves.
//!
//! A session token is required. Every held document is listed at the root,
//! matched by name when the listing carries a `query`; the store has no
//! folders, so the folder listing is empty. Answering at all is what tells the
//! app the instance has a Drive.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::Path as FilePath;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use chrono::SecondsFormat;
use systemprompt_models::documents::DriveEntry;
use systemprompt_models::documents::client::{DriveDocument, DriveListing};

use crate::routes::session;
use crate::state::{InstanceState, lock};

const QUERY: &str = "query";

pub(crate) async fn documents(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
) -> Response {
    if let Err(refused) = session(&state, &headers) {
        return refused;
    }
    let wanted = query.get(QUERY).map(|text| text.trim().to_lowercase());
    let documents = lock(&state.drive)
        .documents
        .values()
        .map(|entry| listed(&state, entry))
        .filter(|document| {
            wanted
                .as_deref()
                .is_none_or(|text| document.name.to_lowercase().contains(text))
        })
        .collect();
    Json(DriveListing {
        folders: Vec::new(),
        documents,
        next_cursor: None,
    })
    .into_response()
}

pub(crate) async fn folders(
    State(state): State<Arc<InstanceState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(refused) = session(&state, &headers) {
        return refused;
    }
    Json(DriveListing::default()).into_response()
}

fn listed(state: &InstanceState, entry: &DriveEntry) -> DriveDocument {
    let name = FilePath::new(&entry.path)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or(&entry.path)
        .to_owned();
    let updated_at = entry.updated_at.to_rfc3339_opts(SecondsFormat::Secs, true);
    DriveDocument {
        id: entry.document_id.clone(),
        folder_id: None,
        name,
        format: entry.format.as_str().to_owned(),
        mime_type: entry.format.mime_type().to_owned(),
        head_revision_id: entry.head_revision_id.clone(),
        template_id: None,
        owner_user_id: state.user.user_id.clone(),
        created_at: updated_at.clone(),
        updated_at,
        size_bytes: Some(entry.size_bytes),
        my_access: None,
    }
}
