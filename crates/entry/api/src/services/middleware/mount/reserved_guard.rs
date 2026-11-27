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
//! `..`, or contains `/`, `\\` or `;`. The site fallback is refused more
//! ([`guarded_site`]): every core route family as well (`/api` and everything
//! under the API base, `/mcp` and `/otel`), whatever the method, so a core
//! path's request and its bearer never reach an install's site. The web root
//! `/` itself is the site fallback's: static content answers it first, and a
//! static miss there (no `index.html`) reaches the fallback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::extract::{OriginalUri, Request};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_extension::reserved::WEB_ROOT_BASE_PATH;
use systemprompt_extension::router::reserved_answering;
use systemprompt_models::api::ApiError;
use systemprompt_models::modules::ApiPaths;

use crate::services::middleware::rate_limit::{LayerReach, layer_with};

pub(super) fn guarded(router: Router, reach: LayerReach) -> Router {
    layer_with(router, reach, axum::middleware::from_fn(refuse_reserved))
}

pub(super) fn guarded_site(router: Router, reach: LayerReach) -> Router {
    layer_with(router, reach, axum::middleware::from_fn(refuse_core_family))
}

fn decoded_path(path: &str) -> Option<String> {
    urlencoding::decode(path)
        .ok()
        .map(std::borrow::Cow::into_owned)
}

fn falls_under_core_family(path: &str) -> bool {
    decoded_path(path).is_none_or(|decoded| {
        decoded == ApiPaths::API_BASE
            || decoded.starts_with(&format!("{}/", ApiPaths::API_BASE))
            || ApiPaths::is_mcp(&decoded)
            || ApiPaths::is_otel(&decoded)
    })
}

fn ambiguous(segment: &str) -> bool {
    segment == "." || segment == ".." || segment.contains(['/', '\\', ';'])
}

fn falls_under_reserved(path: &str, root_admitted: bool) -> bool {
    let mut decoded = Vec::new();
    for segment in path.split('/') {
        match urlencoding::decode(segment) {
            Ok(segment) if ambiguous(&segment) => return true,
            Ok(segment) => decoded.push(segment.into_owned()),
            Err(_) => return true,
        }
    }
    let segments: Vec<&str> = decoded.iter().map(String::as_str).collect();
    reserved_answering(&segments, None)
        .is_some_and(|entry| !(root_admitted && entry.path == WEB_ROOT_BASE_PATH))
}

fn original_path(request: &Request) -> String {
    request
        .extensions()
        .get::<OriginalUri>()
        .map_or_else(|| request.uri().path(), |original| original.path())
        .to_owned()
}

fn not_found(path: String) -> Response {
    (
        StatusCode::NOT_FOUND,
        axum::Json(ApiError::not_found("not found").with_path(path)),
    )
        .into_response()
}

async fn refuse_reserved(request: Request, next: Next) -> Response {
    let path = original_path(&request);
    if falls_under_reserved(&path, false) {
        return not_found(path);
    }
    next.run(request).await
}

async fn refuse_core_family(request: Request, next: Next) -> Response {
    let path = original_path(&request);
    if falls_under_reserved(&path, true) || falls_under_core_family(&path) {
        return not_found(path);
    }
    next.run(request).await
}
