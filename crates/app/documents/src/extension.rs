//! [`DocumentsExtension`] (`documents`): the document store's member.
//!
//! Early it provides the document store cell's read half,
//! [`DocumentsHandlesSlot`], which the files member's seams, the document
//! jobs, the `files-http` routes, the issues commands and the MCP toolsets
//! read. After governance it builds the store from the resolved state, binds
//! the cell and provides the store sealed (`Arc<DocumentsRuntime>`). From
//! the sealed token authority it builds two narrow views and confers each on
//! its one holder: [`DocumentLinkTokens`] on `files-http` and the
//! `DynDriveTokens` view on `gateway-http` (see [`crate::tokens`]).
//!
//! The member is required: the files member's seams read the cell it
//! provides. With `files.documents.enabled` off the store is still built,
//! the bound handles carry no document service, and the routes are absent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::prelude::*;
use systemprompt_extension::reserved::ConferralRow;

use crate::provides::{
    HandlesBinderHold, documents_runtime, drive_tokens, handles_slot, link_tokens,
};
use crate::tokens::DocumentLinkTokens;

pub const DOCUMENTS_EXTENSION: &str = "documents";

/// The member that builds the document store and its token views.
#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentsExtension;

impl Extension for DocumentsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: DOCUMENTS_EXTENSION,
            name: "Documents",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        let hold = Arc::new(HandlesBinderHold::default());
        let early = Arc::clone(&hold);
        c.provide(move |_: &Resolved<'_>| Ok(handles_slot(&early)))
            .provide_sealed_after_governance(move |r: &Resolved<'_>| documents_runtime(&hold, r))
            .provide_conferred(
                ConferralRow::DocumentLinkTokens,
                |r: &Resolved<'_>| -> Result<Arc<DocumentLinkTokens>, ProvideError> {
                    Ok(link_tokens(r))
                },
            )
            .provide_conferred(ConferralRow::DriveTokens, drive_tokens);
        Ok(())
    }
}
