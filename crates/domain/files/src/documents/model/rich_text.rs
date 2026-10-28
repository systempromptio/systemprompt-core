//! The `RichText` model: a document as an ordered list of typed blocks whose
//! text carries character-range marks.
//!
//! Offsets in a [`Mark`] count Unicode scalar values of the block's text, the
//! unit a CRDT text container indexes by. A block's `id` is the stable node
//! id that comments, agents and MCP reads address it by; it survives every
//! edit of the block and is never reused within a document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The version of the `RichText` schema; bumped on any incompatible change.
pub const RICH_TEXT_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RichText {
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Block {
    pub id: String,
    #[serde(flatten)]
    pub kind: BlockKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<Mark>,
}

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
    },
    Quote,
    Code {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
    },
    Table {
        rows: Vec<Vec<RichText>>,
    },
    Image {
        blob: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        alt: String,
    },
    Divider,
    PageBreak,
    /// Native markup the model does not own, kept verbatim and not editable.
    Raw {
        source: String,
    },
}

impl BlockKind {
    /// Whether the block's `text` is editable content; the others keep it
    /// empty.
    #[must_use]
    pub const fn has_text(&self) -> bool {
        matches!(
            self,
            Self::Paragraph
                | Self::Heading { .. }
                | Self::ListItem { .. }
                | Self::Quote
                | Self::Code { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Mark {
    pub start: usize,
    pub end: usize,
    #[serde(flatten)]
    pub kind: MarkKind,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(tag = "mark", rename_all = "snake_case")]
pub enum MarkKind {
    Bold,
    Italic,
    Underline,
    Strike,
    Code,
    Link { href: String },
    Color { value: String },
    Highlight { value: String },
    Comment { thread: String },
}

impl MarkKind {
    /// The key a CRDT text container stores this mark under.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        match self {
            Self::Bold => "bold",
            Self::Italic => "italic",
            Self::Underline => "underline",
            Self::Strike => "strike",
            Self::Code => "code",
            Self::Link { .. } => "link",
            Self::Color { .. } => "color",
            Self::Highlight { .. } => "highlight",
            Self::Comment { .. } => "comment",
        }
    }

    /// The mark's value, empty for the flag marks.
    #[must_use]
    pub fn value(&self) -> &str {
        match self {
            Self::Link { href: v }
            | Self::Color { value: v }
            | Self::Highlight { value: v }
            | Self::Comment { thread: v } => v,
            Self::Bold | Self::Italic | Self::Underline | Self::Strike | Self::Code => "",
        }
    }

    /// Rebuilds a mark from its container key and value.
    #[must_use]
    pub fn from_key(key: &str, value: &str) -> Option<Self> {
        let owned = || value.to_owned();
        Some(match key {
            "bold" => Self::Bold,
            "italic" => Self::Italic,
            "underline" => Self::Underline,
            "strike" => Self::Strike,
            "code" => Self::Code,
            "link" => Self::Link { href: owned() },
            "color" => Self::Color { value: owned() },
            "highlight" => Self::Highlight { value: owned() },
            "comment" => Self::Comment { thread: owned() },
            _ => return None,
        })
    }

    /// Every container key, in a fixed order.
    pub const KEYS: [&'static str; 9] = [
        "bold",
        "italic",
        "underline",
        "strike",
        "code",
        "link",
        "color",
        "highlight",
        "comment",
    ];
}

impl Block {
    #[must_use]
    pub fn new(id: impl Into<String>, kind: BlockKind, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            text: text.into(),
            marks: Vec::new(),
        }
    }

    /// The text's length in Unicode scalar values, the unit of [`Mark`]
    /// offsets.
    #[must_use]
    pub fn len(&self) -> usize {
        self.text.chars().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}
