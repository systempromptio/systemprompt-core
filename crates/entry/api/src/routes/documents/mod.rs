//! `/api/v1/documents` and `/api/v1/document-folders`: the shared document
//! store over HTTP, plus the bridge Drive feed under `/v1/bridge/drive/`.
//!
//! Every handler is thin: it resolves the caller, converts path and query
//! values to typed ids, calls [`DocumentService`] and maps the outcome. The
//! service authorizes every call; a handler never reads the store or the
//! blob store directly. Content is streamed with the head's content hash as
//! the `ETag`, and writes name their base revision in `If-Match`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod caller;
mod commits;
mod content;
mod download;
pub mod drive;
mod error;
mod folders;
mod headers;
mod history;
mod items;
mod sharing;
mod views;

use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post, put};
use systemprompt_files::documents::DocumentService;
use systemprompt_models::auth::TokenAuthority;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AppContext as _, UserProvider};

/// Router-scoped state of the document routes: the one document service and
/// the user provider that resolves a caller's roles.
#[derive(Clone)]
pub struct DocumentsState {
    pub(crate) service: Arc<DocumentService>,
    pub(crate) users: Arc<dyn UserProvider>,
    pub(crate) tokens: Arc<dyn TokenAuthority>,
}

impl std::fmt::Debug for DocumentsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentsState").finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum DocumentsMountError {
    #[error("document routes need a user provider")]
    MissingUserProvider,
}

impl DocumentsState {
    pub fn build(ctx: &AppContext) -> Result<Self, DocumentsMountError> {
        Ok(Self {
            service: document_service(ctx),
            users: ctx
                .user_provider()
                .ok_or(DocumentsMountError::MissingUserProvider)?,
            tokens: ctx.token_authority(),
        })
    }

    #[must_use]
    pub fn service(&self) -> &DocumentService {
        &self.service
    }

    #[must_use]
    pub fn max_body_bytes(&self) -> usize {
        body_limit(&self.service)
    }
}

pub(crate) fn body_limit(service: &DocumentService) -> usize {
    let limit = service.config().max_upload_bytes.saturating_add(64 * 1024);
    usize::try_from(limit).unwrap_or(usize::MAX)
}

pub fn document_service(ctx: &AppContext) -> Arc<DocumentService> {
    Arc::clone(ctx.documents().service())
}

pub fn router(state: DocumentsState) -> Router {
    let limit = state.max_body_bytes();
    Router::new()
        .route("/", get(items::list).post(items::create))
        .route(
            "/{id}",
            get(items::get)
                .head(items::get)
                .patch(items::update)
                .delete(items::delete),
        )
        .route(
            "/{id}/content",
            get(content::read).head(content::read).put(commits::write),
        )
        .route("/{id}/edits", post(commits::edits))
        .route("/{id}/revisions", get(history::revisions))
        .route("/{id}/projection", get(history::projection))
        .route("/{id}/preview", get(history::preview))
        .route(
            "/{id}/grants",
            get(sharing::list_grants)
                .post(sharing::share)
                .delete(sharing::revoke),
        )
        .route(
            "/{id}/presence",
            put(sharing::touch_presence).get(sharing::presence),
        )
        .route(
            "/{id}/lock",
            post(sharing::acquire_lock).delete(sharing::release_lock),
        )
        .layer(DefaultBodyLimit::max(limit))
        .with_state(state)
}

pub fn with_download_tokens(router: Router, state: DocumentsState) -> Router {
    router.layer(axum::middleware::from_fn_with_state(state, download::gate))
}

pub fn folders_router(state: DocumentsState) -> Router {
    Router::new()
        .route("/", get(folders::list).post(folders::create))
        .route(
            "/{id}",
            axum::routing::patch(folders::update).delete(folders::delete),
        )
        .with_state(state)
}
