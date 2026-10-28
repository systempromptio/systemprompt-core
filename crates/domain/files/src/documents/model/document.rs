//! Document-level settings of a `RichText` model: the style catalogue, the
//! theme, numbering definitions and the body's final section.
//!
//! A codec writes a setting only when the model owns it (it is present);
//! an absent setting keeps the base document's part byte for byte, so a
//! template's branding survives every edit that does not restyle it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::layout::{ParaAttrs, Section};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentSettings {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<StyleDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub numbering: Vec<NumberingDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<Section>,
    /// A Markdown document's YAML front matter, verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front_matter: Option<String>,
}

impl DocumentSettings {
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    #[must_use]
    pub fn numbering(&self, id: &str) -> Option<&NumberingDef> {
        self.numbering.iter().find(|def| def.id == id)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StyleKind {
    #[default]
    Paragraph,
    Character,
    Table,
}

/// A named style. `size` is in points and may be fractional (`"10.5"`);
/// `color` is a 6-digit hex colour.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StyleDef {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default)]
    pub kind: StyleKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "ParaAttrs::is_default")]
    pub paragraph: ParaAttrs,
}

/// A font pair and a six-colour accent palette (6-digit hex).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Theme {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading_font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_font: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palette: Vec<String>,
}

/// A list numbering definition; list items reference it by `id`, and each
/// definition is written as its own abstract numbering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NumberingDef {
    pub id: String,
    pub levels: Vec<NumberingLevel>,
}

/// One level. `text` is the label pattern, `%1` standing for this level's
/// counter (`"%1."`, `"%1.%2"`); a bullet's `text` is its glyph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NumberingLevel {
    pub format: NumberFormat,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub start: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indent: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NumberFormat {
    Bullet,
    Decimal,
    LowerLetter,
    UpperLetter,
    LowerRoman,
    UpperRoman,
}

impl NumberFormat {
    #[must_use]
    pub const fn ooxml(self) -> &'static str {
        match self {
            Self::Bullet => "bullet",
            Self::Decimal => "decimal",
            Self::LowerLetter => "lowerLetter",
            Self::UpperLetter => "upperLetter",
            Self::LowerRoman => "lowerRoman",
            Self::UpperRoman => "upperRoman",
        }
    }

    #[must_use]
    pub fn from_ooxml(value: &str) -> Self {
        match value {
            "decimal" | "decimalZero" => Self::Decimal,
            "lowerLetter" => Self::LowerLetter,
            "upperLetter" => Self::UpperLetter,
            "lowerRoman" => Self::LowerRoman,
            "upperRoman" => Self::UpperRoman,
            _ => Self::Bullet,
        }
    }
}

const fn one() -> u32 {
    1
}

const fn is_one(value: &u32) -> bool {
    *value == 1
}
