//! The document store handles and the link token view the document routes
//! read from the kernel state: the cell the `documents` member binds and the
//! narrow view it confers on `files-http`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_documents::tokens::DocumentLinkTokens;
use systemprompt_files::documents::{
    DocumentsHandles, DocumentsHandlesSlot, DocumentsUnbound, bound,
};
use systemprompt_runtime::AppContext;

pub fn documents_handles(ctx: &AppContext) -> Result<Arc<DocumentsHandles>, DocumentsUnbound> {
    let slot = ctx
        .kernel()
        .state()
        .find::<DocumentsHandlesSlot>()
        .ok_or(DocumentsUnbound)?;
    bound(slot).cloned()
}

pub fn link_tokens(ctx: &AppContext) -> Option<Arc<DocumentLinkTokens>> {
    ctx.kernel()
        .state()
        .find::<Arc<DocumentLinkTokens>>()
        .cloned()
}
