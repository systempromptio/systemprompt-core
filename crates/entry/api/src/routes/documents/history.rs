//! Revision history, projections and the sandboxed HTML preview.
//!
//! A `file` document that is a PDF previews as the PDF itself, served
//! inline as `application/pdf` (never as the type it was stored under) for
//! the browser's own viewer, sandboxed by CSP, `nosniff`, and frameable only
//! by the app (`frame-ancestors 'self'`); an image previews inline in the
//! sandboxed HTML.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, header};
use axum::response::{Html, IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use systemprompt_extension::FrameOptionsOverride;
use systemprompt_files::documents::service::{DocumentView, HistoryPage, StreamRequest};
use systemprompt_identifiers::{ContentHash, DocumentRevisionId};
use systemprompt_models::RequestContext;
use systemprompt_models::documents::{DocumentFormat, Projection};
use systemprompt_provider_contracts::FrameOptions;
use systemprompt_traits::FileByteStream;

use super::DocumentsState;
use super::caller::caller;
use super::content::{RevisionQuery, revision_id};
use super::error::{DocumentsHttpError, DocumentsResult};
use super::headers::etag;
use super::items::{cursor, document_id};
use super::views::RevisionJson;
use crate::routes::user_content::{INLINE_PREVIEW_CSP, harden};

const PREVIEW_CSP: &str = "sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data:";
const PDF_MIME: &str = "application/pdf";
const DEFAULT_HISTORY: i64 = 50;
const MAX_HISTORY: i64 = 500;

#[derive(Debug, Deserialize)]
pub(super) struct HistoryQuery {
    limit: Option<i64>,
    before: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct HistoryJson {
    revisions: Vec<RevisionJson>,
}

pub(super) async fn revisions(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> DocumentsResult<Json<HistoryJson>> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let page = HistoryPage {
        limit: query.limit.unwrap_or(DEFAULT_HISTORY).clamp(1, MAX_HISTORY),
        before: cursor(query.before.as_deref())?,
    };
    let revisions = state
        .service
        .history(&who.user, &id, page, &who.trace)
        .await?;
    Ok(Json(HistoryJson {
        revisions: revisions.into_iter().map(Into::into).collect(),
    }))
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ProjectionView {
    #[default]
    Outline,
    Text,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProjectionQuery {
    #[serde(default)]
    view: ProjectionView,
    revision: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(super) enum ProjectionJson {
    Outline {
        revision_id: DocumentRevisionId,
        projection: Projection,
    },
    Text {
        revision_id: DocumentRevisionId,
        text: String,
    },
}

pub(super) async fn projection(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<ProjectionQuery>,
) -> DocumentsResult<Json<ProjectionJson>> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let revision = revision_id(query.revision.as_deref())?;
    let body = match query.view {
        ProjectionView::Outline => {
            let (revision, projection) = state
                .service
                .project(&who.user, &id, revision.as_ref(), &who.trace)
                .await?;
            ProjectionJson::Outline {
                revision_id: revision.id,
                projection,
            }
        },
        ProjectionView::Text => {
            let (revision, text) = state
                .service
                .plain_text(&who.user, &id, revision.as_ref(), &who.trace)
                .await?;
            ProjectionJson::Text {
                revision_id: revision.id,
                text,
            }
        },
    };
    Ok(Json(body))
}

pub(super) async fn preview(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Query(query): Query<RevisionQuery>,
) -> Result<Response, DocumentsHttpError> {
    let who = caller(&state, &req_ctx).await?;
    let id = document_id(&id)?;
    let revision = revision_id(query.revision.as_deref())?;
    let view = state.service.get(&who.user, &id, &who.trace).await?;
    if is_pdf_file(&view) {
        let request = StreamRequest {
            document: &id,
            revision: revision.as_ref(),
            range: None,
        };
        let (revision, read) = state
            .service
            .open_stream(&who.user, request, &who.trace)
            .await?;
        return Ok(pdf_preview(read.body, &revision.content_sha256));
    }
    let (revision, html) = state
        .service
        .preview_html(&who.user, &id, revision.as_ref(), &who.trace)
        .await?;
    let mut response = Html(html).into_response();
    response
        .extensions_mut()
        .insert(FrameOptionsOverride(FrameOptions::SameOrigin));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(PREVIEW_CSP),
    );
    if let Ok(value) = HeaderValue::from_str(&etag(&revision.content_sha256)) {
        headers.insert(header::ETAG, value);
    }
    Ok(response)
}

fn is_pdf_file(view: &DocumentView) -> bool {
    let document = &view.document;
    document.format == DocumentFormat::File
        && (document.mime_type == PDF_MIME || document.name.to_ascii_lowercase().ends_with(".pdf"))
}

fn pdf_preview(body: FileByteStream, hash: &ContentHash) -> Response {
    let mut response = Response::new(Body::from_stream(body));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(PDF_MIME));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline"),
    );
    if let Ok(value) = HeaderValue::from_str(&etag(hash)) {
        headers.insert(header::ETAG, value);
    }
    harden(&mut response, INLINE_PREVIEW_CSP);
    response
}
