//! The document store's HTTP handlers.
//!
//! They are contributed as the [`FilesSurface`] the API mount calls:
//! `/api/v1/documents`,
//! `/api/v1/document-folders`, `/api/v1/uploads`, the anonymous public-link
//! open `/s/{token}` and `/api/v1/issues`.
//!
//! [`FilesHttpSurface::build`] reads the `files-http` route-scoped state: the
//! open document store cell, the member's open [`FilesHttpSettings`], the
//! [`DocumentLinkTokens`](systemprompt_documents::tokens::DocumentLinkTokens)
//! view conferred on `files-http` and the member's own
//! [`FilesHttpRepositories`]. The layering (gates, rate limits, policies)
//! stays the API's.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod documents;
mod error;
pub mod issues;
pub mod uploads;
mod user_content;

use std::sync::Arc;

use systemprompt_documents::http::surface::{
    DocumentRoutes, FilesRouteState, FilesRoutes, FilesSurface, FilesSurfaceError, TokenGatedRoutes,
};
use systemprompt_files::documents::{DocumentsHandles, DocumentsHandlesSlot, bound};
use systemprompt_manifest::Config;
use systemprompt_manifest::profile::UiSessionConfig;
use systemprompt_users::UserRepository;

use self::documents::DocumentsState;
use self::issues::IssuesState;

/// The repositories the files routes read, built once in the member's
/// provide.
#[derive(Debug)]
pub struct FilesHttpRepositories {
    pub users: Arc<UserRepository>,
}

/// The configuration facts the files routes read, copied out of the
/// application configuration by the member's provide.
#[derive(Debug, Clone)]
pub struct FilesHttpSettings {
    pub ui_session: UiSessionConfig,
    pub api_external_url: Arc<str>,
}

impl FilesHttpSettings {
    #[must_use]
    pub fn of(config: &Config) -> Self {
        Self {
            ui_session: config.ui_session,
            api_external_url: Arc::from(config.api_external_url.as_str()),
        }
    }
}

/// The `files-http` member's [`FilesSurface`].
#[derive(Debug, Clone, Copy, Default)]
pub struct FilesHttpSurface;

impl FilesSurface for FilesHttpSurface {
    fn build(&self, state: FilesRouteState<'_>) -> Result<FilesRoutes, FilesSurfaceError> {
        let issues = issues::router(IssuesState::build(&state)?);
        let documents = DocumentsState::build(&state)?.map(document_routes);
        Ok(FilesRoutes { documents, issues })
    }
}

fn document_routes(state: DocumentsState) -> DocumentRoutes {
    let download_gate = state.clone();
    let upload_gate = state.clone();
    DocumentRoutes {
        documents: TokenGatedRoutes {
            user_routes: documents::router(state.clone()),
            token_gate: Box::new(move |routes| {
                documents::with_download_tokens(routes, download_gate)
            }),
        },
        uploads: TokenGatedRoutes {
            user_routes: uploads::router(state.clone()),
            token_gate: Box::new(move |routes| uploads::with_upload_tokens(routes, upload_gate)),
        },
        public_link: documents::public_link::router(
            Arc::clone(&state.service),
            Arc::clone(&state.client_ip),
        ),
        folders: documents::folders_router(state),
    }
}

pub(crate) fn files_handles(
    state: &FilesRouteState<'_>,
) -> Result<Arc<DocumentsHandles>, FilesSurfaceError> {
    let slot = state.resolved.get::<DocumentsHandlesSlot>()?;
    Ok(Arc::clone(bound(&slot)?))
}
