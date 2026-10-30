//! A document's bytes as the live session's model, and back.
//!
//! A new session seeds its engine document with [`import`]: the document as
//! a `RichText` model, through `model::formats` (the inner markup of
//! `<body>` for HTML, the full text for Markdown, and the body of
//! `word/document.xml` or `content.xml` for `docx` and `odt`).
//! [`materialise`] turns the live model back into document bytes on top of
//! the revision the session is based on,
//! keeping everything the model does not own. A session opened before the
//! model existed still holds a flat `ModelView::Body` and materialises the
//! way it always did. [`unchanged_from`] compares a model with its base
//! after both are materialised, since writing normalises markup.
//!
//! A live materialise never fails on content the format cannot hold: a
//! Markdown model is degraded first and each thing left out is logged as a
//! warning. `media` carries the bytes of the images the model places, so a
//! browser-inserted `docx` image saves.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::documents::DocumentFormat;

use super::model::splice_body;
use crate::documents::engine::ModelView;
use crate::documents::model::Media;
use crate::documents::model::formats::materialise_rich_text_live;

pub use crate::documents::model::formats::{ModelError, import_rich_text};

#[cfg(feature = "collab-loro")]
mod structured {
    use super::ModelError;
    use crate::documents::engine::loro::deck::DeckModel;
    use crate::documents::engine::loro::sheet::SheetModel;

    #[cfg(feature = "office")]
    pub(super) fn sheet_bytes(sheets: &SheetModel) -> Result<Vec<u8>, ModelError> {
        Ok(crate::documents::codecs::xlsx::checkpoint_xlsx(sheets)?)
    }

    #[cfg(feature = "office")]
    pub(super) fn deck_bytes(base: &[u8], deck: &DeckModel) -> Result<Vec<u8>, ModelError> {
        Ok(crate::documents::codecs::pptx::lower_deck(base, deck)?)
    }

    #[cfg(not(feature = "office"))]
    pub(super) fn sheet_bytes(_sheets: &SheetModel) -> Result<Vec<u8>, ModelError> {
        Err(ModelError::Unsupported)
    }

    #[cfg(not(feature = "office"))]
    pub(super) fn deck_bytes(_base: &[u8], _deck: &DeckModel) -> Result<Vec<u8>, ModelError> {
        Err(ModelError::Unsupported)
    }
}

pub fn import(format: DocumentFormat, document: &[u8]) -> Result<ModelView, ModelError> {
    #[cfg(all(feature = "office", feature = "collab-loro"))]
    match format {
        DocumentFormat::Xlsx | DocumentFormat::Ods => {
            return Ok(ModelView::Sheet(
                crate::documents::codecs::xlsx::sheet_model_from_xlsx(document)?,
            ));
        },
        DocumentFormat::Pptx => {
            return Ok(ModelView::Deck(crate::documents::codecs::pptx::deck_model(
                document,
            )?));
        },
        _ => {},
    }
    Ok(ModelView::RichText(import_rich_text(format, document)?))
}

pub fn materialise(
    format: DocumentFormat,
    base: &[u8],
    model: &ModelView,
    media: &Media,
) -> Result<Vec<u8>, ModelError> {
    match model {
        ModelView::RichText(rich) => {
            let (bytes, warnings) = materialise_rich_text_live(format, base, rich, media)?;
            for warning in warnings {
                tracing::warn!(%format, warning = %warning, "live checkpoint left out content");
            }
            Ok(bytes)
        },
        ModelView::Body(body) => {
            Ok(splice_body(format, std::str::from_utf8(base)?, body).into_bytes())
        },
        #[cfg(feature = "collab-loro")]
        ModelView::Sheet(sheets) => structured::sheet_bytes(sheets),
        #[cfg(feature = "collab-loro")]
        ModelView::Deck(deck) => structured::deck_bytes(base, deck),
    }
}

pub fn unchanged_from(
    format: DocumentFormat,
    base: &[u8],
    model: &ModelView,
    media: &Media,
) -> Result<bool, ModelError> {
    let next = materialise(format, base, model, media)?;
    Ok(match model {
        ModelView::Body(_) => next == base,
        _ => next == materialise(format, base, &import(format, base)?, media)?,
    })
}
