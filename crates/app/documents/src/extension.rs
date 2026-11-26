//! [`DocumentsExtension`](crate::extension::DocumentsExtension) (`documents`):
//! the document store's member.
//!
//! After governance it builds the store from the resolved state and provides
//! its bound handles sealed (`Arc<DocumentsHandles>`); the host binds them
//! into the `DocumentsHandlesSlot` it seeded, which the files member's seams,
//! the document jobs, the routes, the issues commands and the MCP toolsets
//! read. From the sealed token authority it builds two narrow views (see
//! [`crate::tokens`]):
//! [`DocumentLinkTokens`](crate::tokens::DocumentLinkTokens), conferred on
//! `files-http`, and the `DynDriveTokens` view, conferred on `files-http`
//! for the bridge Drive routes that member builds; the API mounts them under
//! the gateway whatever `gateway-http`'s standing.
//!
//! It contributes the `documents` toolset to the MCP host's `ToolsetSlot`,
//! so a hosted server lists it only in a composition that admits this member.
//!
//! It also provides, sealed, the user lookup a hosted MCP server's document
//! tools read (`Arc<DocumentsMcpUsers>`), so the child host builds no user
//! repository of its own.
//!
//! With `files.documents.enabled` off the store is still built and the
//! bound handles carry no document service.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::prelude::*;
use systemprompt_extension::reserved::ConferralRow;
use systemprompt_mcp_host::contract::ToolsetSlot;
use systemprompt_traits::DynDriveTokens;

use crate::provides::{documents_handles, drive_tokens, link_tokens};
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
        crate::mcp::contribute_toolset(c.slot::<ToolsetSlot>());
        c.provide_sealed_after_governance(documents_handles)
            .provide_sealed(crate::provides::mcp_users)
            .provide_conferred(
                ConferralRow::DocumentLinkTokens,
                |r: &Resolved<'_>| -> Result<Arc<DocumentLinkTokens>, ProvideError> {
                    Ok(link_tokens(r))
                },
            )
            .provide_conferred(
                ConferralRow::DriveTokens,
                |r: &Resolved<'_>| -> Result<DynDriveTokens, ProvideError> { Ok(drive_tokens(r)) },
            );
        Ok(())
    }
}
