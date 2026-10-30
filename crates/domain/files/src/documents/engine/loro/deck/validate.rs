//! The check the apply path runs on a deck document after importing an
//! update, before the update is journalled or broadcast.
//!
//! A deck that does not read, repeats an id within one list, carries a
//! negative frame size or exceeds the slide and shape bounds is refused, so
//! a hostile or buggy replica cannot land a deck the codec cannot lower.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use loro::LoroDoc;

use super::model::DeckModel;
use super::read::{DeckReadError, read};

pub(crate) const MAX_SLIDES: usize = 2_000;
pub(crate) const MAX_SHAPES_PER_SLIDE: usize = 500;

#[derive(Debug, thiserror::Error)]
pub enum DeckInvalid {
    #[error(transparent)]
    Read(#[from] DeckReadError),
    #[error("the deck holds {slides} slides, over the {max} slide bound")]
    TooManySlides { slides: usize, max: usize },
    #[error("slide {slide} holds {shapes} shapes, over the {max} shape bound")]
    TooManyShapes {
        slide: String,
        shapes: usize,
        max: usize,
    },
    #[error("id {id} appears twice in one list")]
    DuplicateId { id: String },
    #[error("shape {shape} has a negative width or height")]
    NegativeFrame { shape: String },
}

pub fn validate_deck_changes(doc: &LoroDoc) -> Result<DeckModel, DeckInvalid> {
    let deck = read(doc)?;
    if deck.slides.len() > MAX_SLIDES {
        return Err(DeckInvalid::TooManySlides {
            slides: deck.slides.len(),
            max: MAX_SLIDES,
        });
    }
    unique(deck.slides.iter().map(|slide| slide.id.as_str()))?;
    for slide in &deck.slides {
        if slide.shapes.len() > MAX_SHAPES_PER_SLIDE {
            return Err(DeckInvalid::TooManyShapes {
                slide: slide.id.clone(),
                shapes: slide.shapes.len(),
                max: MAX_SHAPES_PER_SLIDE,
            });
        }
        unique(slide.shapes.iter().map(|shape| shape.id.as_str()))?;
        for shape in &slide.shapes {
            if shape.frame.is_some_and(|frame| frame.w < 0 || frame.h < 0) {
                return Err(DeckInvalid::NegativeFrame {
                    shape: shape.id.clone(),
                });
            }
            unique(shape.paragraphs.iter().map(|block| block.id.as_str()))?;
        }
    }
    Ok(deck)
}

fn unique<'a>(ids: impl Iterator<Item = &'a str>) -> Result<(), DeckInvalid> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(DeckInvalid::DuplicateId { id: id.to_owned() });
        }
    }
    Ok(())
}
