//! `POST /api/v1/documents/{id}/collab/updates`: one engine update of a live
//! edit session.
//!
//! The update travels either as JSON (`{session_id, vv?, update_b64}`, all
//! binary values standard base64) or, with `Content-Type:
//! application/octet-stream`, as the raw update bytes with `session_id` and
//! the optional base64 `vv` in the query string. Both answer
//! `{seq, vv, missing_b64?}`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Bytes;
use axum::extract::{FromRequest, Path, Query, Request, State};
use axum::http::header;
use axum::{Extension, Json};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use systemprompt_files::documents::collab::ApplyRequest;
use systemprompt_identifiers::DocumentEditSessionId;
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;

use super::DocumentsState;
use super::caller::caller;
use super::collab::decode;
use super::error::DocumentsResult;
use super::items::document_id;

const BINARY: &str = "application/octet-stream";

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
) -> DocumentsResult<Json<AppliedJson>> {
    let who = caller(&state, &req_ctx).await?;
    let binary = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with(BINARY));
    let (session_id, vv, update) = if binary {
        let Query(query) = Query::<BinaryQuery>::try_from_uri(request.uri()).map_err(|error| {
            ApiError::bad_request("session_id is required in the query string").with_source(error)
        })?;
        let body = Bytes::from_request(request, &state)
            .await
            .map_err(|error| {
                ApiError::bad_request("the update body could not be read").with_source(error)
            })?;
        (query.session_id, query.vv, body.to_vec())
    } else {
        let Json(body) = Json::<UpdateBody>::from_request(request, &state).await?;
        (body.session_id, body.vv, decode(&body.update_b64)?)
    };
    state.service.check_collab_update_limit(update.len())?;
    let request = ApplyRequest {
        document_id: document_id(&id)?,
        session_id,
        version: vv.as_deref().map(decode).transpose()?,
        update,
    };
    let applied = state.collab.apply(&who.user, request, &who.trace).await?;
    Ok(Json(AppliedJson {
        seq: applied.seq,
        vv: STANDARD.encode(applied.version),
        missing_b64: applied.missing.map(|bytes| STANDARD.encode(bytes)),
    }))
}
