//! `/api/v1/uploads`: resumable uploads that end in a content-addressed
//! document blob.
//!
//! `POST /` opens a session (`{name, mime, size, sha256}`) and answers its
//! part size and expiry; `PUT /{id}/parts/{n}` streams one part (any order,
//! retry freely); `GET /{id}` reports progress and the missing parts;
//! `POST /{id}/complete` verifies the hash, runs the content checks and
//! answers the `blob_ref`; `DELETE /{id}` aborts; `GET /{id}/content`
//! streams the completed blob with `Range` support. Every route also
//! accepts a signed `access_token` query instead of a session (see
//! `gate`). Handlers are thin over [`UploadService`].
//!
//! [`UploadService`]: systemprompt_files::uploads::UploadService
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod content;
mod error;
mod gate;
mod handlers;
mod principal;
mod views;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post, put};

use crate::routes::documents::DocumentsState;

const PART_BODY_SLACK: u64 = 64 * 1024;

fn routes(state: &DocumentsState) -> Router<DocumentsState> {
    let part_limit = state
        .uploads
        .limits()
        .part_size_bytes
        .saturating_add(PART_BODY_SLACK);
    Router::new()
        .route("/", post(handlers::create))
        .route("/{id}", get(handlers::status).delete(handlers::abort))
        .route(
            "/{id}/parts/{part}",
            put(handlers::put_part).layer(DefaultBodyLimit::max(
                usize::try_from(part_limit).unwrap_or(usize::MAX),
            )),
        )
        .route("/{id}/complete", post(handlers::complete))
        .route(
            "/{id}/content",
            get(content::content).head(content::content),
        )
}

pub fn router(state: DocumentsState) -> Router {
    routes(&state).with_state(state)
}

/// Answers requests that carry a signed upload token before the session
/// middleware `router` is wrapped in.
pub fn with_upload_tokens(router: Router, state: DocumentsState) -> Router {
    let gate_state = gate::GateState {
        tokened: routes(&state).with_state(state.clone()),
        documents: state,
    };
    router.layer(axum::middleware::from_fn_with_state(gate_state, gate::gate))
}
