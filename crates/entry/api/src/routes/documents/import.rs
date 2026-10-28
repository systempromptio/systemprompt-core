//! `POST /import`: a document from a completed upload.
//!
//! The body names the upload's `blob_ref`, the target `folder_id` and the
//! document `name` (its extension picks the format unless `format` is
//! given; `.csv` imports as text). A blob of no editable format becomes a
//! `file` document under the MIME type its upload recorded, listed and
//! downloadable like any other. Only the user who uploaded a blob may
//! import it. A blob past the
//! document upload limit is refused with `413`, never truncated, and the
//! format's codec must parse it whole before the document is created.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;
use systemprompt_files::documents::service::ImportRequest;
use systemprompt_identifiers::{ContentHash, DocumentFolderId};
use systemprompt_models::RequestContext;
use systemprompt_models::documents::DocumentFormat;

use super::DocumentsState;
use super::caller::{api_author, caller};
use super::error::DocumentsResult;
use super::views::CreatedJson;
use crate::error::ApiHttpError;

#[derive(Debug, Deserialize)]
pub(super) struct ImportBody {
    blob_ref: ContentHash,
    #[serde(default)]
    folder_id: Option<DocumentFolderId>,
    name: String,
    #[serde(default)]
    format: Option<DocumentFormat>,
}

pub(super) async fn import(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    headers: HeaderMap,
    body: Result<Json<ImportBody>, JsonRejection>,
) -> DocumentsResult<Response> {
    let Json(body) = body?;
    let who = caller(&state, &req_ctx).await?;
    let limit = state.service.config().max_upload_bytes;
    let (bytes, mime_type) = state
        .uploads
        .read_owned_blob(req_ctx.user_id(), &body.blob_ref, limit)
        .await
        .map_err(ApiHttpError::from)?;
    let request = ImportRequest {
        folder: body.folder_id,
        name: body.name,
        format: body.format,
        mime_type: Some(mime_type),
        bytes,
        author: api_author(&req_ctx, &headers),
    };
    let (document, revision) = state.service.import(&who.user, request, &who.trace).await?;
    let created = CreatedJson {
        document: document.into(),
        revision: revision.into(),
    };
    Ok((StatusCode::CREATED, Json(created)).into_response())
}
