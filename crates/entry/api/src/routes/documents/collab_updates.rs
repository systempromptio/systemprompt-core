//! `POST /api/v1/documents/{id}/collab/updates`: one engine update of a live
//! edit session.
//!
//! The update travels either as JSON (`{session_id, vv?, update_b64}`, all
//! binary values standard base64) or, with `Content-Type:
//! application/octet-stream`, as the raw update bytes with `session_id` and
//! the optional base64 `vv` in the query string. Both answer
//! `{seq, vv, missing_b64?}`, where `missing_b64` is only the operations
//! the client's `vv` lacks, never a snapshot.
//!
//! When another replica holds the document's owner lease the raw request is
//! forwarded there and its answer relayed; a failed forward applies the
//! update here. An update that keeps losing the append race answers `429`
//! with `Retry-After` and `retry_after_ms` so the client backs off and
//! retries instead of rejoining.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Bytes;
use axum::extract::{FromRequest, Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::collab::{
    ApplyRequest, CollabError, FORWARDED_HEADER, ForwardRequest, Forwarded, UpdateRoute,
};
use systemprompt_identifiers::DocumentEditSessionId;
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;

use super::DocumentsState;
use super::caller::caller;
use super::collab::decode;
use super::error::DocumentsResult;
use super::items::document_id;

const BINARY: &str = "application/octet-stream";
const RETRY_AFTER_MS: u64 = 250;

#[derive(Debug, Deserialize)]
pub(super) struct UpdateBody {
    session_id: DocumentEditSessionId,
    vv: Option<String>,
    update_b64: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct BinaryQuery {
    session_id: DocumentEditSessionId,
    vv: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct AppliedJson {
    seq: i64,
    vv: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing_b64: Option<String>,
}

pub(super) async fn updates(
    State(state): State<DocumentsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    request: Request,
) -> DocumentsResult<Response> {
    let who = caller(&state, &req_ctx).await?;
    let document = document_id(&id)?;
    let headers = request.headers().clone();
    let uri = request.uri().clone();
    let content_type = text(&headers, header::CONTENT_TYPE.as_str());
    let binary = content_type.is_some_and(|value| value.starts_with(BINARY));
    let body = Bytes::from_request(request, &state)
        .await
        .map_err(|error| {
            ApiError::bad_request("the update body could not be read").with_source(error)
        })?;
    if headers.get(FORWARDED_HEADER).is_none()
        && let UpdateRoute::Forward { address } = state.collab.update_route(&document).await
    {
        let forward = ForwardRequest {
            address: &address,
            path_and_query: uri
                .path_and_query()
                .map_or_else(|| uri.path(), |pq| pq.as_str()),
            authorization: text(&headers, header::AUTHORIZATION.as_str()),
            cookie: text(&headers, header::COOKIE.as_str()),
            content_type,
            body: body.to_vec(),
        };
        match state.collab.forward_update(forward).await {
            Ok(forwarded) => return Ok(relay(forwarded)),
            Err(error) => tracing::warn!(
                document_id = %document,
                owner = %address,
                error = %error,
                "Collab update not forwarded to its owner; applying it here"
            ),
        }
    }
    let (session_id, vv, update) = if binary {
        let Query(query) = Query::<BinaryQuery>::try_from_uri(&uri).map_err(|error| {
            ApiError::bad_request("session_id is required in the query string").with_source(error)
        })?;
        (query.session_id, query.vv, body.to_vec())
    } else {
        let Json(body) = Json::<UpdateBody>::from_bytes(&body)?;
        (body.session_id, body.vv, decode(&body.update_b64)?)
    };
    state.service.check_collab_update_limit(update.len())?;
    let request = ApplyRequest {
        document_id: document,
        session_id,
        version: vv.as_deref().map(decode).transpose()?,
        update,
    };
    let applied = match state.collab.apply(&who.user, request, &who.trace).await {
        Err(CollabError::UpdateContended { .. }) => return Ok(back_pressure()),
        other => other?,
    };
    Ok(Json(AppliedJson {
        seq: applied.seq,
        vv: STANDARD.encode(applied.version),
        missing_b64: applied.missing.map(|bytes| STANDARD.encode(bytes)),
    })
    .into_response())
}

fn text<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn back_pressure() -> Response {
    let body = serde_json::json!({
        "error_key": "update_contended",
        "retry_after_ms": RETRY_AFTER_MS,
    });
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, "1")],
        Json(body),
    )
        .into_response()
}

fn relay(forwarded: Forwarded) -> Response {
    let status = StatusCode::from_u16(forwarded.status).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = (status, forwarded.body).into_response();
    let headers = response.headers_mut();
    for (name, value) in [
        (header::CONTENT_TYPE, forwarded.content_type),
        (header::RETRY_AFTER, forwarded.retry_after),
    ] {
        if let Some(value) = value.and_then(|value| HeaderValue::from_str(&value).ok()) {
            headers.insert(name, value);
        }
    }
    response
}
