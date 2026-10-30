//! A slide deck held in Loro containers.
//!
//! The root [`LoroMovableList`](loro::LoroMovableList) named
//! [`SLIDES_CONTAINER`] holds one map per slide: `id` (string), `layout`
//! (string, absent when the slide names none) and `shapes`, a movable list
//! of shape maps. A shape map holds `id`, `kind` (one of [`ShapeKind`]'s
//! names), `frame` (a map of `x`, `y`, `w`, `h` in EMU, absent when the
//! shape inherits its frame) and `paragraphs`, a movable list of block maps
//! in exactly the shape [`rich`](super::rich) writes for a document block.
//! A browser editor mirrors these containers; no computed layout is held.
//!
//! [`read`] refuses the whole deck on an entry it cannot read, [`write_deck`]
//! turns a document into a target [`DeckModel`] matching every list entry
//! by id, and [`validate_deck_changes`] is the check the apply path runs on
//! a deck after it imports a client update.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod model;
mod read;
mod validate;
mod write;

pub use model::{DeckModel, DeckShape, DeckSlide, Frame, ShapeKind};
pub use read::{DeckReadError, read};
pub use validate::{DeckInvalid, validate_deck_changes};
pub use write::{insert_paragraph, insert_slide, write_deck};

use loro::{LoroDoc, LoroMovableList};

pub const SLIDES_CONTAINER: &str = "slides";

#[must_use]
pub fn slides(doc: &LoroDoc) -> LoroMovableList {
    doc.get_movable_list(SLIDES_CONTAINER)
}

/// Opens a fresh document holding `deck`, written by the seed peer.
pub fn seed_deck(deck: &DeckModel) -> Result<LoroDoc, super::rich::WriteError> {
    let doc = LoroDoc::new();
    super::rich::configure(&doc);
    doc.set_peer_id(super::SEED_PEER)?;
    write_deck(&doc, deck)?;
    doc.commit();
    doc.set_peer_id(super::replica_peer())?;
    Ok(doc)
}
