//! The character-range marks a `RichText` block's text carries.
//!
//! Flag marks (`bold`, `superscript`, …) carry no value; valued marks carry
//! one string: a link's href, a hex colour, a font family, a size in points
//! (`"10.5"`), a comment thread id, or a field code (`PAGE`, `NUMPAGES`,
//! `DATE`) whose marked text is the field's cached result.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    Font { value: String },
    Size { value: String },
    Superscript,
    Subscript,
    Caps,
    Field { code: String },
}

impl MarkKind {
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
            Self::Font { .. } => "font",
            Self::Size { .. } => "size",
            Self::Superscript => "superscript",
            Self::Subscript => "subscript",
            Self::Caps => "caps",
            Self::Field { .. } => "field",
        }
    }

    #[must_use]
    pub fn value(&self) -> &str {
        match self {
            Self::Link { href: v }
            | Self::Color { value: v }
            | Self::Highlight { value: v }
            | Self::Comment { thread: v }
            | Self::Font { value: v }
            | Self::Size { value: v }
            | Self::Field { code: v } => v,
            Self::Bold
            | Self::Italic
            | Self::Underline
            | Self::Strike
            | Self::Code
            | Self::Superscript
            | Self::Subscript
            | Self::Caps => "",
        }
    }

    #[must_use]
    pub const fn is_flag(&self) -> bool {
        matches!(
            self,
            Self::Bold
                | Self::Italic
                | Self::Underline
                | Self::Strike
                | Self::Code
                | Self::Superscript
                | Self::Subscript
                | Self::Caps
        )
    }

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
            "font" => Self::Font { value: owned() },
            "size" => Self::Size { value: owned() },
            "superscript" => Self::Superscript,
            "subscript" => Self::Subscript,
            "caps" => Self::Caps,
            "field" => Self::Field { code: owned() },
            _ => return None,
        })
    }

    pub const KEYS: [&'static str; 15] = [
        "bold",
        "italic",
        "underline",
        "strike",
        "code",
        "link",
        "color",
        "highlight",
        "comment",
        "font",
        "size",
        "superscript",
        "subscript",
        "caps",
        "field",
    ];
}

pub const FIELD_CODES: [&str; 3] = ["PAGE", "NUMPAGES", "DATE"];
