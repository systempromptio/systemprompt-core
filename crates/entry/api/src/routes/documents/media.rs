//! `GET /{id}/media?ref=<reference>`: an image the document's head holds,
//! for the live editor to draw.
//!
//! The reference is the image block's `blob` (`docx-media:media/image1.png`,
//! or an upload's hash), passed as a query value because it holds slashes.
//! The body is served as user content, with its hash as the `ETag`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::request::Parts;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use systemprompt_models::documents::content_hash;

use super::DocumentsState;
use super::caller::caller;
use super::content::request_context;
use super::error::DocumentsResult;
use super::headers::{hash_etag, none_match};
use super::items::document_id;
use crate::routes::user_content::{USER_CONTENT_CSP, harden};

#[derive(Debug, Deserialize)]
pub(super) struct MediaQuery {
    #[serde(rename = "ref")]
    reference: String,
}

pub(super) async fn image(
    State(state): State<DocumentsState>,
    Path(id): Path<String>,
    Query(query): Query<MediaQuery>,
    parts: Parts,
) -> DocumentsResult<Response> {
    let who = caller(&state, request_context(&parts)?).await?;
    let id = document_id(&id)?;
    let blob = state
        .service
        .held_image(&who.user, &id, &query.reference, &who.trace)
        .await?;
    let tag = hash_etag(&content_hash(&blob.bytes));
    let mut response = if none_match(&parts.headers, &tag) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        blob.bytes.into_response()
    };
    let headers = response.headers_mut();
    for (name, value) in [
        (header::CONTENT_TYPE, blob.mime.as_str()),
        (header::ETAG, tag.as_str()),
        (header::CACHE_CONTROL, "private, no-cache"),
    ] {
        if let Ok(value) = HeaderValue::from_str(value) {
            headers.insert(name, value);
        }
    }
    harden(&mut response, USER_CONTENT_CSP);
    Ok(response)
}
