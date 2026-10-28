//! Opening an upload, storing a part, reading progress, completing and
//! aborting. A part's body is streamed to storage as it arrives; it is
//! never collected in memory.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::body::Body;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{Extensions, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use systemprompt_files::uploads::NewUpload;
use systemprompt_identifiers::DocumentUploadId;
use systemprompt_models::api::ApiError;
use systemprompt_traits::FileStorageError;

use super::principal::uploader;
use super::views::{CompletedJson, CreateBody, UploadJson};
use crate::error::ApiHttpError;
use crate::routes::documents::DocumentsState;

type UploadsResult = Result<Response, ApiHttpError>;

pub(super) fn upload_id(raw: &str) -> Result<DocumentUploadId, ApiHttpError> {
    Ok(DocumentUploadId::try_new(raw)?)
}

pub(super) async fn create(
    State(state): State<DocumentsState>,
    extensions: Extensions,
    body: Result<Json<CreateBody>, JsonRejection>,
) -> UploadsResult {
    let who = uploader(&extensions)?;
    let Json(body) =
        body.map_err(|source| ApiError::bad_request("invalid request body").with_source(source))?;
    let request = NewUpload {
        name: body.name,
        mime: body.mime,
        size: body.size,
        sha256: body.sha256,
    };
    let session = state.uploads.create(&who.user, request, &who.trace).await?;
    Ok((StatusCode::CREATED, Json(UploadJson::from(session))).into_response())
}

pub(super) async fn status(
    State(state): State<DocumentsState>,
    extensions: Extensions,
    Path(id): Path<String>,
) -> UploadsResult {
    let who = uploader(&extensions)?;
    let progress = state.uploads.progress(&who.user, &upload_id(&id)?).await?;
    Ok(Json(UploadJson::from(progress)).into_response())
}

pub(super) async fn put_part(
    State(state): State<DocumentsState>,
    extensions: Extensions,
    Path((id, part)): Path<(String, u32)>,
    body: Body,
) -> UploadsResult {
    let who = uploader(&extensions)?;
    let stream = body
        .into_data_stream()
        .map(|chunk| chunk.map_err(|error| FileStorageError::Io(std::io::Error::other(error))));
    let stored = state
        .uploads
        .put_part(&who.user, &upload_id(&id)?, part, Box::pin(stream))
        .await?;
    let tag = format!("\"{}\"", stored.sha256);
    let mut response = Json(stored).into_response();
    if let Ok(tag) = HeaderValue::from_str(&tag) {
        response.headers_mut().insert(header::ETAG, tag);
    }
    Ok(response)
}

pub(super) async fn complete(
    State(state): State<DocumentsState>,
    extensions: Extensions,
    Path(id): Path<String>,
) -> UploadsResult {
    let who = uploader(&extensions)?;
    let done = state.uploads.complete(&who.user, &upload_id(&id)?).await?;
    Ok(Json(CompletedJson::from(done)).into_response())
}

pub(super) async fn abort(
    State(state): State<DocumentsState>,
    extensions: Extensions,
    Path(id): Path<String>,
) -> UploadsResult {
    let who = uploader(&extensions)?;
    state.uploads.abort(&who.user, &upload_id(&id)?).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
