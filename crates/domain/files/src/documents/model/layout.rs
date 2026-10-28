//! The layout attributes of a `RichText` block: paragraph formatting, table
//! geometry and cell styling, image placement and section page setup.
//!
//! Every length is in points (1/72 inch) and every image dimension in CSS
//! pixels, so an agent never reasons in twips or EMUs; a codec converts on
//! write and back on import. Every attribute is optional and an all-default
//! value is omitted from JSON, so a model written before these attributes
//! existed reads unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::rich_text::RichText;

/// Paragraph-level formatting of a text block.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParaAttrs {
    /// A paragraph style id from the document's style catalogue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indent: Option<Indent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_before: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_after: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub keep_with_next: bool,
}

impl ParaAttrs {
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    Center,
    Right,
    Justify,
}

/// Indentation in points; `first_line` is negative for a hanging indent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Indent {
    #[serde(default)]
    pub left: u32,
    #[serde(default)]
    pub right: u32,
    #[serde(default)]
    pub first_line: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VAlign {
    Top,
    Center,
    Bottom,
}

/// A table's geometry and styling beside its `rows` of cell content.
/// `cells` is sparse: a missing row or cell has default styling.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TableProps {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<Column>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub header_rows: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<Vec<CellProps>>,
    /// A table style id from the document's style catalogue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
}

impl TableProps {
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    #[must_use]
    pub fn cell(&self, row: usize, column: usize) -> CellProps {
        self.cells
            .get(row)
            .and_then(|cells| cells.get(column))
            .cloned()
            .unwrap_or_default()
    }
}

/// A column width in points; `None` lets the table size it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Column {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
}

/// One cell's styling. A cell covered by another's `colspan`/`rowspan` keeps
/// its slot in `rows` with empty content and is not written. A span of 0
/// reads as 1.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CellProps {
    /// Background fill as a 6-digit hex colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shade: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valign: Option<VAlign>,
    #[serde(default, skip_serializing_if = "is_one")]
    pub colspan: u32,
    #[serde(default, skip_serializing_if = "is_one")]
    pub rowspan: u32,
}

impl CellProps {
    #[must_use]
    pub const fn colspan(&self) -> u32 {
        if self.colspan == 0 { 1 } else { self.colspan }
    }

    #[must_use]
    pub const fn rowspan(&self) -> u32 {
        if self.rowspan == 0 { 1 } else { self.rowspan }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImageWrap {
    #[default]
    Inline,
    Square,
    TopBottom,
    Behind,
    InFront,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CalloutTone {
    Info,
    Success,
    Warning,
    Danger,
    Note,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

/// A section's page setup and running content. Size and margins are in
/// points; `None` keeps the base document's value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Section {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<PageSize>,
    #[serde(default, skip_serializing_if = "is_portrait")]
    pub orientation: Orientation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margins: Option<Margins>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<RichText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footer: Option<RichText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_page_header: Option<RichText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_page_footer: Option<RichText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub even_header: Option<RichText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub even_footer: Option<RichText>,
}

impl Section {
    /// Every header and footer this section defines.
    pub fn running_parts(&self) -> impl Iterator<Item = &RichText> {
        [
            &self.header,
            &self.footer,
            &self.first_page_header,
            &self.first_page_footer,
            &self.even_header,
            &self.even_footer,
        ]
        .into_iter()
        .flatten()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PageSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Margins {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

const fn is_zero(value: &u32) -> bool {
    *value == 0
}

const fn is_one(value: &u32) -> bool {
    *value <= 1
}

const fn is_portrait(value: &Orientation) -> bool {
    matches!(value, Orientation::Portrait)
}
