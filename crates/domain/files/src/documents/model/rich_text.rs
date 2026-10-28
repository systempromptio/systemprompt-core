//! The `RichText` model: a document as an ordered list of typed blocks whose
//! text carries character-range marks.
//!
//! Offsets in a [`Mark`] count Unicode scalar values of the block's text, the
//! unit a CRDT text container indexes by. A block's `id` is the stable node
//! id that comments, agents and MCP reads address it by; it survives every
//! edit of the block and is never reused within a document.
//!
//! Version 2 adds paragraph attributes ([`ParaAttrs`]), numbered lists bound
//! to a numbering definition, table geometry and cell styling, image
//! placement and captions, callouts, sections, a table of contents and the
//! document-level [`DocumentSettings`]. Every addition is optional, so a
//! version 1 model reads as the same version 2 model.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::document::DocumentSettings;
use super::layout::{CalloutTone, ImageWrap, ParaAttrs, Section, TableProps};
pub use super::mark_kind::MarkKind;

pub const RICH_TEXT_VERSION: u32 = 2;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RichText {
    pub blocks: Vec<Block>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<Box<DocumentSettings>>,
}

impl RichText {
    #[must_use]
    pub fn running(&self) -> Vec<&Self> {
        self.document
            .as_deref()
            .and_then(|settings| settings.section.as_ref())
            .map(|section| section.running_parts().collect())
            .unwrap_or_default()
    }

    pub fn running_mut(&mut self) -> Vec<&mut Self> {
        self.document
            .as_deref_mut()
            .and_then(|settings| settings.section.as_mut())
            .map(Section::running_parts_mut)
            .unwrap_or_default()
    }

    #[must_use]
    pub const fn of(blocks: Vec<Block>) -> Self {
        Self {
            blocks,
            document: None,
        }
    }

    #[must_use]
    pub fn settings(&self) -> DocumentSettings {
        self.document.as_deref().cloned().unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Block {
    pub id: String,
    #[serde(flatten)]
    pub kind: BlockKind,
    #[serde(default, skip_serializing_if = "ParaAttrs::is_default")]
    pub attrs: ParaAttrs,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<Mark>,
}

/// A block's type and attributes.
///
/// Paragraph, heading, list item, quote, code and callout blocks carry text;
/// `Raw` is native markup the model does not own, kept verbatim and never
/// authored by a live update. A `section_break` ends a section and carries that
/// section's page setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockKind {
    Paragraph,
    Heading {
        level: u8,
    },
    ListItem {
        level: u8,
        ordered: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        numbering: Option<String>,
        #[schemars(description = "A task-list item's box: `Some(true)` ticked, `Some(false)` open.")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checked: Option<bool>,
    },
    Quote,
    Code {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
    },
    Table {
        rows: Vec<Vec<RichText>>,
        #[serde(default, skip_serializing_if = "TableProps::is_default")]
        props: TableProps,
    },
    Image {
        blob: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        alt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caption: Option<String>,
        #[serde(default, skip_serializing_if = "is_inline")]
        wrap: ImageWrap,
    },
    Callout {
        tone: CalloutTone,
    },
    #[schemars(description = "A footnote's text; `footnote` marks in the body reference its label.")]
    Footnote {
        label: String,
    },
    Divider,
    PageBreak,
    SectionBreak {
        section: Box<Section>,
    },
    Toc {
        #[serde(default = "toc_levels")]
        levels: u8,
    },
    Raw {
        source: String,
    },
}

impl BlockKind {
    #[must_use]
    pub fn table(rows: Vec<Vec<RichText>>) -> Self {
        Self::Table {
            rows,
            props: TableProps::default(),
        }
    }

    #[must_use]
    pub const fn image(blob: String, alt: String) -> Self {
        Self::Image {
            blob,
            alt,
            width: None,
            height: None,
            caption: None,
            wrap: ImageWrap::Inline,
        }
    }

    #[must_use]
    pub const fn list_item(level: u8, ordered: bool) -> Self {
        Self::ListItem {
            level,
            ordered,
            numbering: None,
            checked: None,
        }
    }

    #[must_use]
    pub fn nested(&self) -> Vec<&RichText> {
        match self {
            Self::Table { rows, .. } => rows.iter().flatten().collect(),
            Self::SectionBreak { section } => section.running_parts().collect(),
            _ => Vec::new(),
        }
    }

    #[must_use]
    pub fn nested_mut(&mut self) -> Vec<&mut RichText> {
        match self {
            Self::Table { rows, .. } => rows.iter_mut().flatten().collect(),
            Self::SectionBreak { section } => section.running_parts_mut(),
            _ => Vec::new(),
        }
    }

    #[must_use]
    pub const fn has_text(&self) -> bool {
        matches!(
            self,
            Self::Paragraph
                | Self::Heading { .. }
                | Self::ListItem { .. }
                | Self::Quote
                | Self::Code { .. }
                | Self::Callout { .. }
                | Self::Footnote { .. }
        )
    }
}

const fn is_inline(wrap: &ImageWrap) -> bool {
    matches!(wrap, ImageWrap::Inline)
}

const fn toc_levels() -> u8 {
    3
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Mark {
    pub start: usize,
    pub end: usize,
    #[serde(flatten)]
    pub kind: MarkKind,
}

impl Block {
    #[must_use]
    pub fn new(id: impl Into<String>, kind: BlockKind, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            attrs: ParaAttrs::default(),
            text: text.into(),
            marks: Vec::new(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.text.chars().count()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}
