//! Deck ops in a live session: an op batch translated into one engine
//! update against the `slides` model (shapes with EMU frames and rich
//! paragraphs, see
//! [`engine::loro::deck`](crate::documents::engine::loro::deck)).
//!
//! [`deck_update`] applies `insert_slide`, `delete` (a slide or a
//! paragraph), `replace_text`, `str_replace` and `insert_after` to a fork
//! of the live document under the peer assigned to the caller, commits it
//! with origin `agent` and returns the update. Node ids name the deck as it
//! stands when each op runs, in the codec's `slide[N]/shape[S]/p[P]` form.
//! Any other op answers `unsupported_format`, so the caller commits the
//! batch to the head. [`edit`] keeps answering `unsupported_format` until
//! the session holds a deck model, at which point it forks the live
//! document and calls [`deck_update`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::documents::EditOp;
#[cfg(feature = "collab-loro")]
use systemprompt_models::documents::NodeKind;

use crate::documents::collab::CollabError;
use crate::documents::error::DocumentsError;
use crate::documents::models::Document;
use crate::documents::service::LiveEdit;

#[cfg(feature = "collab-loro")]
pub use live::{DeckOpError, deck_update};

pub(super) fn edit(document: &Document, _ops: &[EditOp]) -> Result<Option<LiveEdit>, CollabError> {
    Err(CollabError::Documents(DocumentsError::UnsupportedFormat {
        format: document.format,
    }))
}

#[cfg(feature = "collab-loro")]
const fn is_list_item(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::ListItem)
}

#[cfg(feature = "collab-loro")]
mod live;
