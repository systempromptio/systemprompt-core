//! The bridge Drive feed under `/v1/bridge/drive/`.
//!
//! The gateway mounts the feed. This module also carries the document store
//! handles the API reads from the kernel state: the
//! cell the `documents` member binds and the link token view it confers on
//! `files-http`, and the host handles the `files-http` surface is built
//! with. The document, folder, upload, public-link and issue
//! handlers are the `files-http` member's.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod drive;
mod drive_commit;
mod drive_local;
mod drive_token;
mod handles;

use std::sync::Arc;

use systemprompt_documents::http::error::DocumentsHttpError;
use systemprompt_documents::http::surface::FilesHostHandles;
use systemprompt_files::documents::{DocumentService, DocumentsUnbound};
use systemprompt_runtime::AppContext;
use systemprompt_traits::AppContext as _;

pub use handles::{documents_handles, link_tokens};

use crate::kernel::{ApiHttpError, resolve_client_ip};

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum DocumentsMountError {
    #[error("document routes need the document store")]
    Unbound(#[from] DocumentsUnbound),
    #[error("document routes need the document store enabled")]
    Disabled,
    #[error("document routes need the documents member's link token view")]
    MissingLinkTokens,
}

#[must_use]
pub fn files_host_handles(ctx: &AppContext) -> FilesHostHandles {
    let trusted = Arc::new(ctx.config().trusted_proxies.clone());
    FilesHostHandles {
        users: ctx.user_provider(),
        client_ip: Arc::new(move |headers, connect_info| {
            resolve_client_ip(headers, connect_info, &trusted)
        }),
    }
}

pub fn document_service(ctx: &AppContext) -> Option<Arc<DocumentService>> {
    documents_handles(ctx).ok()?.service.clone()
}

impl From<ApiHttpError> for DocumentsHttpError {
    fn from(error: ApiHttpError) -> Self {
        Self::from(error.into_inner())
    }
}
