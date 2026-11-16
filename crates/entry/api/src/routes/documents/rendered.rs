//! The rendered view of an HTML document: the revision's stored bytes,
//! served inline as the page its author designed.
//!
//! The body is the upload itself, so the response is the trust boundary. Its
//! CSP carries `sandbox` without `allow-same-origin`, which makes the page
//! an opaque-origin document wherever it is opened, framed or not: it holds
//! no session, reads no cookie and reaches no route of this installation.
//! `files.documents.renderedHtml` decides whether its scripts run and
//! whether it loads `https:` assets; both are off unless the profile turns
//! them on. The response is never cached, so a profile change applies to the
//! next load. Only the app may frame it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::request::Parts;
use axum::http::{HeaderValue, Method, header};
use axum::response::Response;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::service::StreamRequest;
use systemprompt_models::documents::DocumentFormat;

use super::DocumentsState;
use super::caller::caller;
use super::content::{RevisionQuery, request_context, revision_id};
use super::error::DocumentsResult;
use super::headers::set_revision_tags;
use super::items::document_id;
use crate::routes::user_content::{harden, rendered_html_csp};

const RENDERED_OP: &str = "rendered";

pub(super) async fn rendered(
    State(state): State<DocumentsState>,
    Path(id): Path<String>,
    Query(query): Query<RevisionQuery>,
    parts: Parts,
) -> DocumentsResult<Response> {
    let who = caller(&state, request_context(&parts)?).await?;
    let id = document_id(&id)?;
    let revision = revision_id(query.revision.as_deref())?;
    let view = state.service.get(&who.user, &id, &who.trace).await?;
    if view.document.format != DocumentFormat::Html {
        return Err(DocumentsError::UnsupportedOp {
            op: RENDERED_OP,
            format: view.document.format,
            accepted_kinds: &[],
        }
        .into());
    }
    let request = StreamRequest {
        document: &id,
        revision: revision.as_ref(),
        range: None,
    };
    let (revision, read) = state
        .service
        .open_stream(&who.user, request, &who.trace)
        .await?;
    let body = if parts.method == Method::HEAD {
        Body::empty()
    } else {
        Body::from_stream(read.body)
    };
    let mut response = Response::new(body);
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    set_revision_tags(headers, &revision.id, &revision.content_sha256);
    let policy = &state.service.config().rendered_html;
    harden(
        &mut response,
        rendered_html_csp(policy.scripts, policy.remote_assets),
    );
    Ok(response)
}
