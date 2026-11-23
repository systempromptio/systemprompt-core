//! `/api/v1/documents` and `/api/v1/document-folders`: the shared document
//! store over HTTP, plus the bridge Drive feed under `/v1/bridge/drive/`.
//!
//! Every handler is thin: it resolves the caller, converts path and query
//! values to typed ids, calls [`DocumentService`] and maps the outcome. The
//! service authorizes every call; a handler never reads the store or the
//! blob store directly. Content is streamed with the quoted revision id as
//! the `ETag` and the content hash in `x-document-content-sha256`, and writes
//! name that revision in `If-Match`. With the
//! `collab` feature the `/{id}/collab/*` routes co-edit HTML and Markdown
//! documents live. `/{id}/live` is the document's live channel and
//! `/{id}/collab/awareness` posts awareness; both work without an engine.
//! Sharing, transfer, delete and agent-cap writes sit behind the RFC 9470
//! step-up a person's UI session must meet (see `step_up.rs`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod access;
pub mod actor_scope;
mod agent_cap;
mod caller;
#[cfg(feature = "collab")]
mod collab;
#[cfg(feature = "collab")]
pub mod collab_updates;
mod comments;
mod commits;
mod content;
mod download;
pub mod drive;
mod drive_commit;
mod drive_local;
mod drive_token;
mod error;
mod export;
mod external;
mod folder_sharing;
mod folders;
mod handles;
mod headers;
mod history;
mod import;
mod items;
mod items_multipart;
mod lifecycle;
mod listing;
mod live;
mod media;
mod people;
mod proposals;
pub mod public_link;
mod read;
mod rendered;
mod requests;
mod schema;
mod sharing;
mod step_up;
mod views;

use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{MethodRouter, delete, get, patch, post, put};
use systemprompt_documents::tokens::DocumentLinkTokens;
#[cfg(feature = "collab")]
use systemprompt_files::documents::collab::CollabService;
use systemprompt_files::documents::{DocumentService, DocumentsUnbound};

pub use handles::{documents_handles, link_tokens};
use systemprompt_files::uploads::UploadService;
use systemprompt_manifest::profile::UiSessionConfig;
use systemprompt_runtime::AppContext;
#[cfg(feature = "collab")]
use systemprompt_traits::BackgroundTasks;
use systemprompt_traits::{AppContext as _, DynContextDirectory, UserProvider};
use systemprompt_users::UserRepository;

/// Router-scoped state of the document and upload routes: the one document
/// service, the upload service, the user provider that resolves a caller's
/// roles and the UI-session lifetimes the step-up reads.
#[derive(Clone)]
pub struct DocumentsState {
    pub(crate) service: Arc<DocumentService>,
    pub(crate) uploads: Arc<UploadService>,
    pub(crate) users: Arc<dyn UserProvider>,
    pub(crate) user_repository: Arc<UserRepository>,
    pub(crate) tokens: Arc<DocumentLinkTokens>,
    pub(crate) contexts: DynContextDirectory,
    pub(crate) ui_session: UiSessionConfig,
    #[cfg(feature = "collab")]
    pub(crate) collab: Arc<CollabService>,
    #[cfg(feature = "collab")]
    pub(crate) background: BackgroundTasks,
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
    #[error("document routes need the document store")]
    Unbound(#[from] DocumentsUnbound),
    #[error("document routes need the document store enabled")]
    Disabled,
    #[error("document routes need the documents member's link token view")]
    MissingLinkTokens,
}

impl DocumentsState {
    pub fn build(ctx: &AppContext) -> Result<Self, DocumentsMountError> {
        let handles = documents_handles(ctx)?;
        Ok(Self {
            service: handles
                .service
                .clone()
                .ok_or(DocumentsMountError::Disabled)?,
            uploads: Arc::clone(&handles.uploads),
            users: ctx
                .user_provider()
                .ok_or(DocumentsMountError::MissingUserProvider)?,
            user_repository: Arc::clone(ctx.user_repository()),
            tokens: link_tokens(ctx).ok_or(DocumentsMountError::MissingLinkTokens)?,
            contexts: Arc::clone(&handles.contexts),
            ui_session: ctx.config().ui_session,
            #[cfg(feature = "collab")]
            collab: Arc::clone(&handles.collab),
            #[cfg(feature = "collab")]
            background: ctx.background_tasks().clone(),
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

pub fn document_service(ctx: &AppContext) -> Option<Arc<DocumentService>> {
    documents_handles(ctx).ok()?.service.clone()
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

fn step_up_of(
    state: &DocumentsState,
) -> impl Fn(MethodRouter<DocumentsState>) -> MethodRouter<DocumentsState> + use<> {
    let ui_session = state.ui_session;
    move |route| {
        route.route_layer(axum::middleware::from_fn_with_state(
            ui_session,
            step_up::require_recent_sign_in,
        ))
    }
}

fn with_views(router: Router<DocumentsState>) -> Router<DocumentsState> {
    router
        .route("/{id}/projection", get(history::projection))
        .route("/{id}/preview", get(history::preview))
        .route(
            "/{id}/rendered",
            get(rendered::rendered).head(rendered::rendered),
        )
}

pub fn router(state: DocumentsState) -> Router {
    let stepped_up = step_up_of(&state);
    external::with_public_links(with_views(with_collab(Router::new())), &state)
        .route("/", get(listing::list).post(items::create))
        .route("/import", post(import::import))
        .route("/schema", get(schema::schema))
        .route(
            "/{id}",
            get(items::get)
                .head(items::get)
                .patch(items::update)
                .merge(stepped_up(delete(items::delete))),
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
        .route("/{id}/comments/{comment_id}/reopen", post(comments::reopen))
        .route(
            "/{id}/comments/{comment_id}",
            patch(comments::edit).delete(comments::delete),
        )
        .route("/{id}/export/docx", get(export::docx))
        .route("/{id}/export/markdown", get(export::markdown))
        .route("/{id}/media", get(media::image))
        .route("/{id}/access", get(access::access))
        .route(
            "/{id}/agent-cap",
            get(agent_cap::get_document).merge(stepped_up(put(agent_cap::put_document))),
        )
        .route("/{id}/transfer", stepped_up(post(access::transfer)))
        .route("/{id}/grants/all", stepped_up(delete(access::revoke_all)))
        .route("/{id}/untrash", post(lifecycle::untrash))
        .route("/{id}/copy", post(lifecycle::copy))
        .route("/subject-kinds", get(access::subject_kinds))
        .route("/subjects", get(access::subjects))
        .route("/people", get(people::people))
        .route("/{id}/access-requests", post(requests::create))
        .route(
            "/{id}/access-requests/{rid}/grant",
            stepped_up(post(requests::grant)),
        )
        .route(
            "/{id}/access-requests/{rid}/decline",
            post(requests::decline),
        )
        .route("/{id}/revisions", get(history::revisions))
        .route("/{id}/proposals", get(proposals::list))
        .route("/{id}/proposals/{pid}", get(proposals::get))
        .route("/{id}/proposals/{pid}/accept", post(proposals::accept))
        .route("/{id}/proposals/{pid}/reject", post(proposals::reject))
        .route(
            "/{id}/grants",
            get(sharing::list_grants)
                .merge(stepped_up(post(sharing::share).delete(sharing::revoke))),
        )
        .route(
            "/{id}/presence",
            put(sharing::touch_presence).get(sharing::presence),
        )
        .route(
            "/{id}/lock",
            post(sharing::acquire_lock).delete(sharing::release_lock),
        )
        .layer(DefaultBodyLimit::max(state.max_body_bytes()))
        .route_layer(axum::middleware::from_fn(actor_scope::scope))
        .with_state(state)
}

pub fn with_download_tokens(router: Router, state: DocumentsState) -> Router {
    router.layer(axum::middleware::from_fn_with_state(state, download::gate))
}

pub fn folders_router(state: DocumentsState) -> Router {
    let stepped_up = step_up_of(&state);
    Router::new()
        .route("/", get(folders::list).post(folders::create))
        .route(
            "/{id}",
            patch(folders::update).merge(stepped_up(delete(folders::delete))),
        )
        .route(
            "/{id}/grants",
            get(folder_sharing::list_grants).merge(stepped_up(
                post(folder_sharing::share).delete(folder_sharing::revoke),
            )),
        )
        .route("/{id}/access", get(folder_sharing::access))
        .route(
            "/{id}/agent-cap",
            get(agent_cap::get_folder).merge(stepped_up(put(agent_cap::put_folder))),
        )
        .route_layer(axum::middleware::from_fn(actor_scope::scope))
        .with_state(state)
}
