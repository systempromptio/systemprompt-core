//! Revision history, projections and the sandboxed HTML preview.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, header};
use axum::response::{Html, IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use systemprompt_extension::FrameOptionsOverride;
use systemprompt_files::documents::service::HistoryPage;
use systemprompt_identifiers::DocumentRevisionId;
use systemprompt_models::RequestContext;
use systemprompt_models::documents::Projection;
use systemprompt_provider_contracts::FrameOptions;

use super::DocumentsState;
use super::caller::caller;
use super::content::{RevisionQuery, revision_id};
use super::error::{DocumentsHttpError, DocumentsResult};
use super::headers::etag;
use super::items::{cursor, document_id};
use super::views::RevisionJson;

const PREVIEW_CSP: &str = "sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data:";
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
