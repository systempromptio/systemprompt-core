//! `/api/v1/documents` and `/api/v1/document-folders`: the shared document
//! store over HTTP, plus the bridge Drive feed under `/v1/bridge/drive/`.
//!
//! Every handler is thin: it resolves the caller, converts path and query
//! values to typed ids, calls [`DocumentService`] and maps the outcome. The
//! service authorizes every call; a handler never reads the store or the
//! blob store directly. Content is streamed with the head's content hash as
//! the `ETag`, and writes name their base revision in `If-Match`. With the
//! `collab` feature the `/{id}/collab/*` routes co-edit HTML and Markdown
//! documents live. `/{id}/live` is the document's live channel and
//! `/{id}/collab/awareness` posts awareness; both work without an engine.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod access;
mod agent_cap;
mod caller;
#[cfg(feature = "collab")]
mod collab;
#[cfg(feature = "collab")]
mod collab_updates;
mod comments;
mod commits;
mod content;
mod download;
pub mod drive;
mod error;
mod export;
mod folder_sharing;
mod folders;
mod headers;
mod history;
mod import;
mod items;
mod items_multipart;
mod lifecycle;
mod listing;
mod live;
mod proposals;
mod read;
mod requests;
mod schema;
mod sharing;
mod views;

use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, patch, post, put};
use systemprompt_files::documents::DocumentService;
#[cfg(feature = "collab")]
use systemprompt_files::documents::collab::CollabService;
use systemprompt_files::uploads::UploadService;
use systemprompt_models::auth::TokenAuthority;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AppContext as _, UserProvider};
use systemprompt_users::UserRepository;

/// Router-scoped state of the document and upload routes: the one document
/// service, the upload service and the user provider that resolves a
/// caller's roles.
#[derive(Clone)]
pub struct DocumentsState {
    pub(crate) service: Arc<DocumentService>,
    pub(crate) uploads: Arc<UploadService>,
    pub(crate) users: Arc<dyn UserProvider>,
    pub(crate) user_repository: Arc<UserRepository>,
    pub(crate) tokens: Arc<dyn TokenAuthority>,
    #[cfg(feature = "collab")]
    pub(crate) collab: Arc<CollabService>,
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
            uploads: Arc::clone(ctx.documents().uploads()),
            users: ctx
                .user_provider()
                .ok_or(DocumentsMountError::MissingUserProvider)?,
            user_repository: Arc::clone(ctx.user_repository()),
            tokens: ctx.token_authority(),
            #[cfg(feature = "collab")]
            collab: collab_service(ctx),
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

#[cfg(feature = "collab")]
fn collab_service(ctx: &AppContext) -> Arc<CollabService> {
    Arc::clone(ctx.documents().collab())
}

#[cfg(feature = "collab")]
fn with_collab(router: Router<DocumentsState>) -> Router<DocumentsState> {
    router
        .route("/{id}/collab/join", post(collab::join))
        .route("/{id}/collab/updates", post(collab_updates::updates))
        .route("/{id}/collab/leave", post(collab::leave))
        .route("/{id}/collab/cursor", post(collab::cursor))
}

#[cfg(not(feature = "collab"))]
const fn with_collab(router: Router<DocumentsState>) -> Router<DocumentsState> {
    router
}

pub fn router(state: DocumentsState) -> Router {
    let limit = state.max_body_bytes();
    with_collab(Router::new())
        .route("/", get(listing::list).post(items::create))
        .route("/import", post(import::import))
        .route("/schema", get(schema::schema))
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
        .route("/{id}/read", get(read::read))
        .route("/{id}/live", get(live::live))
        .route("/{id}/collab/awareness", post(live::awareness))
        .route("/{id}/comments", get(comments::list).post(comments::create))
        .route(
            "/{id}/comments/{comment_id}/resolve",
            post(comments::resolve),
        )
        .route(
            "/{id}/comments/{comment_id}",
            patch(comments::edit).delete(comments::delete),
        )
        .route("/{id}/export/docx", get(export::docx))
        .route("/{id}/access", get(access::access))
        .route(
            "/{id}/agent-cap",
            get(agent_cap::get_document).put(agent_cap::put_document),
        )
        .route("/{id}/transfer", post(access::transfer))
        .route("/{id}/grants/all", delete(access::revoke_all))
        .route("/{id}/untrash", post(lifecycle::untrash))
        .route("/{id}/copy", post(lifecycle::copy))
        .route("/subject-kinds", get(access::subject_kinds))
        .route("/subjects", get(access::subjects))
        .route("/{id}/access-requests", post(requests::create))
        .route("/{id}/access-requests/{rid}/grant", post(requests::grant))
        .route(
            "/{id}/access-requests/{rid}/decline",
            post(requests::decline),
        )
        .route("/{id}/revisions", get(history::revisions))
        .route("/{id}/proposals", get(proposals::list))
        .route("/{id}/proposals/{pid}", get(proposals::get))
        .route("/{id}/proposals/{pid}/accept", post(proposals::accept))
        .route("/{id}/proposals/{pid}/reject", post(proposals::reject))
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
        .route("/{id}", patch(folders::update).delete(folders::delete))
        .route(
            "/{id}/grants",
            get(folder_sharing::list_grants)
                .post(folder_sharing::share)
                .delete(folder_sharing::revoke),
        )
        .route("/{id}/access", get(folder_sharing::access))
        .route(
            "/{id}/agent-cap",
            get(agent_cap::get_folder).put(agent_cap::put_folder),
        )
        .with_state(state)
}
