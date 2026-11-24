//! The `files-http` host handles and the document link token view the API
//! reads from the kernel state.
//!
//! The document, folder, upload, public-link, issue and bridge Drive handlers
//! are the `files-http` member's; the API reads the bound document store
//! through [`systemprompt_documents::bound_handles`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_documents::http::error::DocumentsHttpError;
use systemprompt_documents::http::surface::FilesHostHandles;
use systemprompt_documents::tokens::DocumentLinkTokens;
use systemprompt_manifest::Config;
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_runtime::AppContext;
use systemprompt_traits::AppContext as _;

use crate::kernel::{ApiHttpError, resolve_client_ip};

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum DocumentsMountError {
    #[error("document routes need the document store enabled")]
    Disabled,
    #[error("document routes need the documents member's link token view")]
    MissingLinkTokens,
}

pub fn files_host_handles(ctx: &AppContext) -> Result<FilesHostHandles, MissingDependency> {
    let trusted = Arc::new(
        ctx.kernel()
            .state()
            .get::<Arc<Config>>()?
            .trusted_proxies
            .clone(),
    );
    Ok(FilesHostHandles {
        users: ctx.user_provider(),
        client_ip: Arc::new(move |headers, connect_info| {
            resolve_client_ip(headers, connect_info, &trusted)
        }),
    })
}

pub fn link_tokens(ctx: &AppContext) -> Option<Arc<DocumentLinkTokens>> {
    ctx.kernel()
        .state()
        .find::<Arc<DocumentLinkTokens>>()
        .cloned()
}

impl From<ApiHttpError> for DocumentsHttpError {
    fn from(error: ApiHttpError) -> Self {
        Self::from(error.into_inner())
    }
}
