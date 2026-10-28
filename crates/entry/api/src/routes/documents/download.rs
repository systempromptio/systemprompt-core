//! Signed download links for `GET /api/v1/documents/{id}/content`.
//!
//! A request whose query carries `access_token` is answered here, before the
//! session middleware: the token must verify for the download audience, and
//! its `purpose` must name exactly the requested document and `revision`.
//! The read is then authorized as the token's subject, so revoking the
//! subject's access also revokes the link; `format=pdf` serves the same
//! revision rendered as PDF. Every other request passes through to the
//! session-authenticated routes unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_files::documents::agents::act_as;
use systemprompt_identifiers::{DocumentRevisionId, TraceId, UserId};
use systemprompt_models::api::ApiError;
use systemprompt_models::auth::JwtAudience;
use systemprompt_runtime::documents::actor::bearer_actor;
use systemprompt_runtime::documents::download::{
    DOWNLOAD_AUDIENCE, DOWNLOAD_FORMAT_QUERY, DOWNLOAD_TOKEN_QUERY, download_agent,
};

use super::DocumentsState;
use super::caller::{Caller, load};
use super::content::{ContentRequest, rendition, serve};
use super::error::DocumentsResult;
use super::items::document_id;
use crate::error::ApiHttpError;

struct DownloadQuery {
    token: String,
    revision: Option<String>,
    format: Option<String>,
}

fn download_query(request: &Request) -> Option<DownloadQuery> {
    let query = request.uri().query()?;
    let mut token = None;
    let mut revision = None;
    let mut format = None;
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        if key == DOWNLOAD_TOKEN_QUERY {
            token = Some(value.into_owned());
        } else if key == "revision" {
            revision = Some(value.into_owned());
        } else if key == DOWNLOAD_FORMAT_QUERY {
            format = Some(value.into_owned());
        }
    }
    Some(DownloadQuery {
        token: token?,
        revision,
        format,
    })
}

fn content_path(request: &Request) -> Option<String> {
    let path = request.uri().path().trim_matches('/');
    let (id, rest) = path.split_once('/')?;
    (rest == "content").then(|| id.to_owned())
}

pub(super) async fn gate(
    State(state): State<DocumentsState>,
    request: Request,
    next: Next,
) -> Response {
    let readable = matches!(*request.method(), Method::GET | Method::HEAD);
    let target = readable
        .then(|| content_path(&request).zip(download_query(&request)))
        .flatten();
    let Some((id, query)) = target else {
        return next.run(request).await;
    };
    let (parts, _) = request.into_parts();
    match download(&state, &parts.method, &parts.headers, &id, query).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn download(
    state: &DocumentsState,
    method: &Method,
    headers: &HeaderMap,
    id: &str,
    query: DownloadQuery,
) -> DocumentsResult<Response> {
    let audience = [JwtAudience::Resource(DOWNLOAD_AUDIENCE.to_owned())];
    let claims = state
        .tokens
        .verify_bearer(&query.token, &audience)
        .map_err(|source| {
            ApiError::unauthorized("download link is invalid or has expired").with_source(source)
        })?;
    let id = document_id(id)?;
    let revision = query
        .revision
        .as_deref()
        .map(DocumentRevisionId::try_new)
        .transpose()
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::forbidden("a download link names its revision"))?;
    let purpose = claims.custom.purpose.as_deref().unwrap_or_default();
    let Some(agent) = download_agent(purpose, &id, &revision) else {
        return Err(
            ApiHttpError::forbidden("download link is for another document revision").into(),
        );
    };
    let actor = bearer_actor(Some(agent), None, None);
    let subject = UserId::try_new(claims.registered.sub).map_err(ApiError::from)?;
    let who = Caller {
        user: load(state.users.as_ref(), &subject).await?,
        trace: TraceId::generate(),
        actor: actor.clone(),
    };
    let request = ContentRequest {
        who: &who,
        method,
        headers,
        id: &id,
        revision: Some(revision),
        rendition: rendition(query.format.as_deref())?,
    };
    act_as(actor, serve(state, request)).await
}
