//! The answer an install route or the site fallback gives a reserved path:
//! `404`, whether or not the path's owner is composed.
//!
//! An install parameter route admitted beside a reserved literal
//! (`/api/public/{short_code}` beside `/api/public/gateway`) and the site
//! fallback both match requests the router does not send to a reserved route:
//! a reserved path whose owner is not composed, an unmatched path inside a
//! reserved namespace, or a percent-encoded spelling of a reserved literal.
//! [`refuse_reserved`] decodes each segment of the original request path and
//! answers `404` when the decoded path falls under a reserved entry
//! (`systemprompt_extension::router::reserved_answering`); a segment that does
//! not decode to UTF-8 is refused the same way, and so is a segment that a
//! normalising proxy could read differently: one that decodes to `.` or
//! `..`, or contains `/`, `\\` or `;`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::extract::{OriginalUri, Request};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_extension::router::reserved_answering;
use systemprompt_models::api::ApiError;

use crate::services::middleware::rate_limit::{LayerReach, layer_with};

pub(super) fn guarded(router: Router, reach: LayerReach) -> Router {
    layer_with(router, reach, axum::middleware::from_fn(refuse_reserved))
}

fn ambiguous(segment: &str) -> bool {
    segment == "." || segment == ".." || segment.contains(['/', '\\', ';'])
}

fn falls_under_reserved(path: &str) -> bool {
    let mut decoded = Vec::new();
    for segment in path.split('/') {
        match urlencoding::decode(segment) {
            Ok(segment) if ambiguous(&segment) => return true,
            Ok(segment) => decoded.push(segment.into_owned()),
            Err(_) => return true,
        }
    }
    let segments: Vec<&str> = decoded.iter().map(String::as_str).collect();
    reserved_answering(&segments, None).is_some()
}

async fn refuse_reserved(request: Request, next: Next) -> Response {
    let path = request
        .extensions()
        .get::<OriginalUri>()
        .map_or_else(|| request.uri().path(), |original| original.path())
        .to_owned();
    if falls_under_reserved(&path) {
        return (
            StatusCode::NOT_FOUND,
            axum::Json(ApiError::not_found("not found").with_path(path)),
        )
            .into_response();
    }
    next.run(request).await
}
