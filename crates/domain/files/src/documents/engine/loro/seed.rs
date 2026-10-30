//! A Loro document read as a [`ModelView`] and seeded from one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{LoroDoc, LoroText, PeerID};

use super::{BODY_CONTAINER, SEED_PEER, deck, rich, sheet, write_model};
use crate::documents::engine::{EngineError, ModelView};
use crate::documents::model::RichText;

pub fn export(doc: &LoroDoc) -> Result<ModelView, EngineError> {
    if sheet::is_sheet_doc(doc) {
        return sheet::read(doc)
            .map(ModelView::Sheet)
            .map_err(|e| EngineError::Model(Box::new(e)));
    }
    if !deck::slides(doc).is_empty() {
        return deck::read(doc)
            .map(ModelView::Deck)
            .map_err(|e| EngineError::Model(Box::new(e)));
    }
    let body = body_text(doc);
    if rich::blocks(doc).is_empty() && !body.is_empty() {
        Ok(ModelView::Body(body))
    } else {
        Ok(ModelView::RichText(
            rich::read(doc).map_err(|e| EngineError::Model(Box::new(e)))?,
        ))
    }
}

#[must_use]
pub fn body_text(doc: &LoroDoc) -> String {
    doc.get_text(BODY_CONTAINER).to_string()
}

pub fn seed_model(model: &ModelView) -> Result<LoroDoc, rich::WriteError> {
    let doc = LoroDoc::new();
    rich::configure(&doc);
    doc.set_peer_id(SEED_PEER)?;
    match model {
        ModelView::RichText(rich) => write_model(&doc, rich)?,
        ModelView::Sheet(sheets) => sheet::write_sheets(&doc, sheets)?,
        ModelView::Deck(slides) => deck::write_deck(&doc, slides)?,
        ModelView::Body(body) => {
            let text: LoroText = doc.get_text(BODY_CONTAINER);
            text.insert(0, body)?;
        },
    }
    doc.commit();
    doc.set_peer_id(replica_peer())?;
    Ok(doc)
}

pub fn seed(body: &str) -> Result<LoroDoc, rich::WriteError> {
    seed_model(&ModelView::Body(body.to_owned()))
}

pub fn seed_rich(model: &RichText) -> Result<LoroDoc, rich::WriteError> {
    seed_model(&ModelView::RichText(model.clone()))
}

#[must_use]
pub fn replica_peer() -> PeerID {
    uuid::Uuid::new_v4().as_u64_pair().0.max(1)
}
