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
mod content;
mod documents;
pub mod drive;
mod error;
mod folders;
mod headers;
mod sharing;
mod views;

use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post, put};
use systemprompt_files::documents::blob_store::BlobStore;
use systemprompt_files::documents::codecs::CodecRegistry;
use systemprompt_files::documents::repository::DocumentRepositories;
use systemprompt_files::documents::templates::{
    TemplateCatalogError, TemplateRegistry, load_catalog,
};
use systemprompt_files::documents::{
    DocumentAccessRepositories, DocumentAccessService, DocumentEvents, DocumentService,
    DocumentServiceParts,
};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AppContext as _, UserProvider};

/// Router-scoped state of the document routes: the one document service and
/// the user provider that resolves a caller's roles.
#[derive(Clone)]
pub struct DocumentsState {
    pub(crate) service: Arc<DocumentService>,
    pub(crate) users: Arc<dyn UserProvider>,
}

impl std::fmt::Debug for DocumentsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentsState").finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DocumentsMountError {
    #[error("document routes need a user provider")]
    MissingUserProvider,
    #[error("document template catalog could not be loaded")]
    Catalog(#[from] TemplateCatalogError),
}

impl DocumentsState {
    pub fn build(ctx: &AppContext) -> Result<Self, DocumentsMountError> {
        Ok(Self {
            service: document_service(ctx)?,
            users: ctx
                .user_provider()
                .ok_or(DocumentsMountError::MissingUserProvider)?,
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

pub fn document_service(ctx: &AppContext) -> Result<Arc<DocumentService>, DocumentsMountError> {
    let db = ctx.db_pool();
    let repos = Arc::new(DocumentRepositories::new(db));
    let access = Arc::new(DocumentAccessService::new(
        DocumentAccessRepositories {
            grants: Arc::clone(&repos.grants),
            folders: Arc::clone(&repos.folders),
            documents: Arc::clone(&repos.documents),
            access_entities: crate::repository::access_control(db),
        },
        Arc::clone(ctx.authz_hook()),
        Arc::new(ctx.subject_providers().clone()),
    ));
    let services_root = systemprompt_extension::ExtensionContext::services_root(ctx);
    let catalog = load_catalog(&services_root.path)?;
    Ok(Arc::new(DocumentService::new(DocumentServiceParts {
        blobs: Arc::new(BlobStore::new(
            Arc::clone(ctx.file_storage()),
            Arc::clone(&repos.blobs),
        )),
        repos,
        codecs: Arc::new(CodecRegistry::discover()),
        templates: Arc::new(TemplateRegistry::discover(catalog)),
        access,
        events: Arc::new(DocumentEvents::new(Arc::new(ctx.event_router().clone()))),
        config: *ctx.files_config().documents_config(),
    })))
}

pub fn router(state: DocumentsState) -> Router {
    let limit = state.max_body_bytes();
    Router::new()
        .route("/", get(documents::list).post(documents::create))
        .route(
            "/{id}",
            get(documents::get)
                .head(documents::get)
                .patch(documents::update)
                .delete(documents::delete),
        )
        .route(
            "/{id}/content",
            get(content::read).head(content::read).put(content::write),
        )
        .route("/{id}/edits", post(content::edits))
        .route("/{id}/revisions", get(content::revisions))
        .route("/{id}/projection", get(content::projection))
        .route("/{id}/preview", get(content::preview))
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

pub fn folders_router(state: DocumentsState) -> Router {
    Router::new()
        .route("/", get(folders::list).post(folders::create))
        .route("/{id}", axum::routing::patch(folders::update).delete(folders::delete))
        .with_state(state)
}
