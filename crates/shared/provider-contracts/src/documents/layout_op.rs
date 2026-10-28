//! The layout operations of rich text documents (`docx`, and `md` where it
//! can hold the result): paragraph formatting, any block, table geometry,
//! sections, styles and the theme.
//!
//! Their payloads are `RichText` v2 JSON (the schema served at
//! `/api/v1/documents/schema` and in the documents guide), carried here as
//! JSON because this crate sits below the model: `set_paragraph.attrs` is
//! a paragraph-attributes object, `insert_block.block` one block without
//! its `id`, `set_section.section` a section, `set_style.style` a style
//! definition and `apply_theme.theme` a theme. `set_paragraph` and
//! `update_table` address a node and guard it like every rich op;
//! `insert_block` inserts after its anchor; `set_style`, `set_section` and
//! `apply_theme` act on the whole document and name no node.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::edit_op::NodeTarget;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetParagraph {
    pub node: NodeTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_hash: Option<String>,
    #[schemars(
        description = "Paragraph attributes: `style`, `align`, `indent`, `space_before`, `space_after`, `keep_with_next`. Merged into the block's attributes; a `null` value clears that attribute."
    )]
    // JSON: RichText v2 JSON; this crate sits below the model that parses it.
    pub attrs: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InsertBlock {
    pub after: NodeTarget,
    #[schemars(description = "One `RichText` block of any type, without `id`.")]
    // JSON: RichText v2 JSON; this crate sits below the model that parses it.
    pub block: serde_json::Value,
}

/// One cell's new content and styling; an absent field is left as it is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CellUpdate {
    pub row: usize,
    pub col: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[schemars(description = "Cell styling: `shade`, `align`, `valign`, `colspan`, `rowspan`.")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    // JSON: RichText v2 JSON; this crate sits below the model that parses it.
    pub props: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateTable {
    pub table: NodeTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<CellUpdate>,
    #[schemars(description = "Column widths in points, one per column; `null` sizes to content.")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widths: Option<Vec<Option<u32>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_rows: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetSection {
    #[schemars(
        description = "Page size, orientation, margins, headers and footers of the document's last section."
    )]
    // JSON: RichText v2 JSON; this crate sits below the model that parses it.
    pub section: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetStyle {
    #[schemars(description = "One style definition, added or replacing the style with its `id`.")]
    // JSON: RichText v2 JSON; this crate sits below the model that parses it.
    pub style: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ApplyTheme {
    #[schemars(description = "`heading_font`, `body_font` and up to six `palette` colours.")]
    // JSON: RichText v2 JSON; this crate sits below the model that parses it.
    pub theme: serde_json::Value,
}
