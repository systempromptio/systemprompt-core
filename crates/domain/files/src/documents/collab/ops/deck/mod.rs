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
//! batch to the head. The Loro engine's `update_from_ops` calls
//! [`deck_update`] for a deck session; `ops::structured` logs the update.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "collab-loro")]
use systemprompt_models::documents::NodeKind;

#[cfg(feature = "collab-loro")]
pub(crate) use live::{DeckOpError, deck_update};

#[cfg(feature = "collab-loro")]
const fn is_list_item(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::ListItem)
}

#[cfg(feature = "collab-loro")]
mod live;
#[cfg(feature = "collab-loro")]
mod slide;
