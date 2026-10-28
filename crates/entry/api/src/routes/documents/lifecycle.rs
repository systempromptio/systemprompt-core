//! Restoring a trashed document and copying a document's head into a new
//! document the caller owns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;
use systemprompt_files::documents::service::CopyRequest;
use systemprompt_identifiers::DocumentFolderId;
use systemprompt_models::RequestContext;

use super::DocumentsState;
use super::caller::{api_author, caller};
use super::error::DocumentsResult;
use super::items::document_id;
use super::views::{CreatedJson, DocumentJson};

pub(super) async fn restore(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<Json<DocumentJson>> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let document = state
        .service
        .restore_document(&who.user, &id, &who.trace)
        .await?;
    Ok(Json(document.into()))
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct CopyBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    folder_id: Option<DocumentFolderId>,
}

pub(super) async fn copy(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Option<Json<CopyBody>>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let Json(body) = body.unwrap_or_default();
    let request = CopyRequest {
        source: document_id(&id)?,
        name: body.name,
        folder: body.folder_id,
        author: api_author(&req_ctx, &headers),
    };
    let (document, revision) = state.service.copy(&who.user, request, &who.trace).await?;
    let created = CreatedJson {
        document: document.into(),
        revision: revision.into(),
    };
    Ok((StatusCode::CREATED, Json(created)).into_response())
}
