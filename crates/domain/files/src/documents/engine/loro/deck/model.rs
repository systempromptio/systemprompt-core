//! The engine-agnostic slide deck a deck document holds.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::model::Block;

/// A shape's position and size in EMU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

/// What a shape holds. `Title` and `Body` are text placeholders; only
/// text-bearing shapes carry paragraphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    Title,
    Body,
    Text,
    Table,
    Image,
    Other,
}

impl ShapeKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Body => "body",
            Self::Text => "text",
            Self::Table => "table",
            Self::Image => "image",
            Self::Other => "other",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Self::Title,
            Self::Body,
            Self::Text,
            Self::Table,
            Self::Image,
            Self::Other,
        ]
        .into_iter()
        .find(|kind| kind.name() == name)
    }

    #[must_use]
    pub const fn holds_text(self) -> bool {
        matches!(self, Self::Title | Self::Body | Self::Text)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckShape {
    pub id: String,
    pub kind: ShapeKind,
    pub frame: Option<Frame>,
    pub paragraphs: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckSlide {
    pub id: String,
    pub layout: Option<String>,
    pub shapes: Vec<DeckShape>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeckModel {
    pub slides: Vec<DeckSlide>,
}
