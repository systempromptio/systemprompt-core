//! `/api/v1/issues`: bug and feedback reports over HTTP.
//!
//! A signed-in user files a report (`POST /`), lists their own (`GET /`),
//! reads one (`GET /{id}`), declares an attachment upload
//! (`POST /{id}/attachments`) and attaches it once completed
//! (`POST /{id}/attachments/{upload_id}`). The triage roles, or an admin,
//! list every report (`GET /admin`) and move one through triage
//! (`PATCH /admin/{id}`). Handlers are thin over [`IssueService`].
//!
//! [`IssueService`]: systemprompt_files::issues::IssueService
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod admin;
mod attachments;
mod error;
mod handlers;
mod views;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, patch, post};
use systemprompt_documents::tokens::DocumentLinkTokens;
use systemprompt_files::issues::IssueService;
use systemprompt_files::uploads::UploadService;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::AuthenticatedUser;

use crate::http::error::ApiHttpError;
use systemprompt_documents::http::surface::{FilesRouteState, FilesSurfaceError};
use systemprompt_manifest::Config;

use crate::http::files_handles;

/// Router-scoped state of the issue routes: the issue and upload services,
/// the authority that signs upload links and the external base URL they
/// point at.
#[derive(Clone)]
pub struct IssuesState {
    pub(crate) issues: Arc<IssueService>,
    pub(crate) uploads: Arc<UploadService>,
    pub(crate) tokens: Arc<DocumentLinkTokens>,
    pub(crate) base_url: Arc<str>,
}

impl std::fmt::Debug for IssuesState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuesState").finish_non_exhaustive()
    }
}

impl IssuesState {
    pub fn build(state: &FilesRouteState<'_>) -> Result<Self, FilesSurfaceError> {
        let handles = files_handles(state)?;
        let config = state.resolved.get::<Arc<Config>>()?;
        Ok(Self {
            issues: Arc::clone(&handles.issues),
            uploads: Arc::clone(&handles.uploads),
            tokens: state.resolved.get::<Arc<DocumentLinkTokens>>()?,
            base_url: Arc::from(config.api_external_url.as_str()),
        })
    }
}

fn caller(req_ctx: &RequestContext) -> Result<&AuthenticatedUser, ApiHttpError> {
    req_ctx
        .user
        .as_ref()
        .ok_or_else(|| ApiHttpError::unauthorized("issue routes need an authenticated user"))
}

pub fn router(state: IssuesState) -> Router {
    Router::new()
        .route("/", get(handlers::list).post(handlers::file))
        .route("/admin", get(admin::list))
        .route("/admin/{id}", patch(admin::set_status))
        .route("/{id}", get(handlers::find))
        .route("/{id}/attachments", post(attachments::declare))
        .route("/{id}/attachments/{upload_id}", post(attachments::attach))
        .with_state(state)
}
