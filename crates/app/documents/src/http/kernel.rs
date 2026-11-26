//! The `documents` member's kernel-facing work.
//!
//! [`DocumentsKernelService`] implements
//! [`DocumentsKernel`]: the member provides it sealed and the API calls it
//! with the kernel's own resolution. It builds the internal link router over
//! the bound document store, the link token authority the member provides and
//! the reloadable services state; it registers the document folders as
//! access-control entities at boot; and it renders the `endpoints.documents`
//! discovery block ([`DocumentsDiscovery`]). A store bound with documents
//! disabled refuses the internal link router (`Disabled`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use systemprompt_extension::prelude::Resolved;
use systemprompt_extension::router::{DocumentsKernel, DynSubjectCharge, FilesSurfaceError};
use systemprompt_loader::ServicesState;
use systemprompt_manifest::Config;

use super::internal_links::links_router;
use crate::discovery::DocumentsDiscovery;
use crate::links::LinkMinter;
use crate::tokens::DocumentLinkTokens;

/// The `documents` member's [`DocumentsKernel`].
#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentsKernelService;

#[async_trait::async_trait]
impl DocumentsKernel for DocumentsKernelService {
    fn internal_links(
        &self,
        kernel: &Resolved<'_>,
        throttle: Option<DynSubjectCharge>,
    ) -> Result<Router, FilesSurfaceError> {
        let handles =
            crate::resolved_handles(kernel).map_err(|_unbound| FilesSurfaceError::Unbound)?;
        let documents = handles.service.clone().ok_or(FilesSurfaceError::Disabled)?;
        let config = kernel.get::<Arc<Config>>()?;
        let minter = LinkMinter {
            tokens: kernel.get::<Arc<DocumentLinkTokens>>()?,
            documents,
            uploads: Arc::clone(&handles.uploads),
            services: kernel.get::<ServicesState>()?,
            api_external_url: config.api_external_url.clone(),
        };
        Ok(links_router(minter, throttle))
    }

    async fn reconcile_folder_entities(
        &self,
        kernel: &Resolved<'_>,
    ) -> Result<u64, FilesSurfaceError> {
        let access = Arc::clone(
            &crate::resolved_handles(kernel)
                .map_err(|_unbound| FilesSurfaceError::Unbound)?
                .access,
        );
        access
            .reconcile_folder_entities()
            .await
            .map_err(|source| FilesSurfaceError::Reconcile(Box::new(source)))
    }

    // JSON: the block is merged verbatim into the discovery document, whose
    // wire shape this member owns.
    fn discovery(
        &self,
        kernel: &Resolved<'_>,
        base: &str,
    ) -> Result<serde_json::Value, FilesSurfaceError> {
        serde_json::to_value(DocumentsDiscovery::of_resolved(kernel, base))
            .map_err(FilesSurfaceError::Discovery)
    }
}
