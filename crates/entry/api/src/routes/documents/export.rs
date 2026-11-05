//! `GET /{id}/export/docx`: a `docx` document's head revision with its open
//! comment threads written as native Word comments. `GET
//! /{id}/export/markdown`: the head revision as Markdown, with what Markdown
//! cannot hold named in `x-export-left-out`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Extension;
use axum::extract::{Path, State};
use axum::http::{HeaderName, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use systemprompt_files::documents::pdf::MARKDOWN_MIME_TYPE;
use systemprompt_models::RequestContext;
use systemprompt_models::documents::DocumentFormat;

use super::DocumentsState;
use super::caller::caller;
use super::error::DocumentsResult;
use super::headers::content_disposition;
use super::items::document_id;

const LEFT_OUT: HeaderName = HeaderName::from_static("x-export-left-out");

pub(super) async fn docx(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let exported = state
        .service
        .export_with_comments(&who.user, &id, &who.trace)
        .await?;
    let mut response = exported.bytes.into_response();
    let headers = response.headers_mut();
    let mut set = |name, value: String| {
        if let Ok(value) = HeaderValue::from_str(&value) {
            headers.insert(name, value);
        }
    };
    set(
        header::CONTENT_TYPE,
        DocumentFormat::Docx.mime_type().to_owned(),
    );
    set(header::CACHE_CONTROL, "private, no-cache".to_owned());
    set(
        header::CONTENT_DISPOSITION,
        content_disposition(&exported.name),
    );
    Ok(response)
}

pub(super) async fn markdown(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let exported = state
        .service
        .export_markdown(&who.user, &id, &who.trace)
        .await?;
    let left_out = exported.left_out.join("; ");
    let mut response = exported.markdown.into_response();
    let headers = response.headers_mut();
    let mut set = |name, value: String| {
        if let Ok(value) = HeaderValue::from_str(&value) {
            headers.insert(name, value);
        }
    };
    set(header::CONTENT_TYPE, MARKDOWN_MIME_TYPE.to_owned());
    set(header::CACHE_CONTROL, "private, no-cache".to_owned());
    set(
        header::CONTENT_DISPOSITION,
        content_disposition(&exported.file_name),
    );
    if !left_out.is_empty() {
        set(LEFT_OUT, left_out.replace(|c: char| !c.is_ascii(), "?"));
    }
    Ok(response)
}
