//! Edit ops written into a live session instead of onto the head.
//!
//! While a document has an open edit session, an op batch from MCP
//! (`documents_edit`) or the API (`/edits`) is translated into one engine
//! update and goes through the path a browser update takes: logged as the
//! session's next update, imported by every replica and broadcast on the
//! document's live channel. The update is written under an engine peer the
//! server assigns to the caller for this batch (`document_collab_peers`), so
//! it never reuses a browser's peer, and is logged with the actor the call
//! runs as, so an agent's batch folds into the checkpoint actor like any
//! other update. No revision lands here; the session's checkpoint commits
//! the result, which is why `follow_head` never sees an ops write as an
//! outside revision.
//!
//! Translation is per model: [`OpsFormat::Rich`] covers text documents
//! (`rich`); spreadsheets (`sheet`) and decks (`deck`) answer
//! `unsupported_format` until their models are live, and an unsupported
//! batch or a document with no open session is answered with `None`, so the
//! caller commits it to the head as before.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod deck;
mod rich;
mod sheet;

use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentFormat, EditOp};

use super::{CollabError, CollabService};
use crate::documents::error::DocumentsError;
use crate::documents::service::{LiveEdit, LiveEditRequest};

/// The model family an op batch is translated against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpsFormat {
    Rich,
    Sheet,
    Deck,
}

impl OpsFormat {
    #[must_use]
    pub const fn of(format: DocumentFormat) -> Self {
        match format {
            DocumentFormat::Xlsx | DocumentFormat::Ods => Self::Sheet,
            DocumentFormat::Pptx | DocumentFormat::Odp => Self::Deck,
            _ => Self::Rich,
        }
    }
}

impl CollabService {
    /// Writes `request` into the document's open live session, or answers
    /// `None` when there is none or its model has no live ops yet.
    pub async fn edit_live(
        &self,
        who: &AuthenticatedUser,
        request: LiveEditRequest,
        trace: &TraceId,
    ) -> Result<Option<LiveEdit>, CollabError> {
        if self.engine.is_none() || !request.ops.iter().all(is_live_op) {
            return Ok(None);
        }
        let document = self
            .documents
            .admit_live_ops(who, &request.document_id, &request.ops, trace)
            .await?;
        let outcome = match OpsFormat::of(document.format) {
            OpsFormat::Rich => self.edit_rich(who, &document, request).await,
            OpsFormat::Sheet => sheet::edit(&document, &request.ops),
            OpsFormat::Deck => deck::edit(&document, &request.ops),
        };
        match outcome {
            Err(CollabError::Documents(DocumentsError::UnsupportedFormat { .. })) => Ok(None),
            other => other,
        }
    }
}

/// Whole-document and revision ops (`set_text`, `set_html`, `restore`,
/// `insert_from_revision`) replace or expand against revisions, so they keep
/// committing to the head and the session follows them.
const fn is_live_op(op: &EditOp) -> bool {
    !matches!(
        op,
        EditOp::SetText { .. }
            | EditOp::SetHtml { .. }
            | EditOp::Restore { .. }
            | EditOp::InsertFromRevision { .. }
    )
}
