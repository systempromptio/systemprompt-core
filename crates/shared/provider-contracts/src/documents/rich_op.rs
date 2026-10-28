//! The rich-text and table operations, applied to a text document through
//! its `RichText` model rather than by a codec splicing native markup.
//!
//! Each op names one block of the projection the caller read: by id with the
//! `expected_hash` it read, or by quote. `set_marks` adds or removes inline
//! marks over the whole block or over the `nth` occurrence of `find` in it;
//! `set_block_type` retypes a text block; the table ops address a table node
//! (or any node inside it) and count rows and columns from zero, `at`
//! defaulting to the end. `insert_table` and `insert_image` add a block
//! after the anchor. A format whose model cannot hold the result refuses the
//! op with an unsupported-op error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::num::NonZeroUsize;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::edit_op::{EditOp, NodeTarget};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MarkSpec {
    pub mark: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SetMarks {
    pub node: NodeTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub find: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nth: Option<NonZeroUsize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<MarkSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<String>,
}

/// A text block's type, as `set_block_type` names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TextBlockType {
    Paragraph,
    Heading {
        level: u8,
    },
    ListItem {
        #[serde(default)]
        level: u8,
        #[serde(default)]
        ordered: bool,
    },
    Quote,
    Code {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SetBlockType {
    pub node: NodeTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_hash: Option<String>,
    pub block: TextBlockType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InsertTable {
    pub after: NodeTarget,
    pub rows: Vec<Vec<String>>,
}

/// A row or column added to a table; `cells` fills it from the start and
/// missing cells are empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TableInsert {
    pub table: NodeTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<String>,
}

/// Deletes one row, one column, or (with neither) the whole table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TableDelete {
    pub table: NodeTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InsertImage {
    pub after: NodeTarget,
    pub blob: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub alt: String,
}

/// The node a rich op addresses, and its guard when the op rewrites that
/// node; an op that only inserts after its anchor has no guard.
#[derive(Debug)]
pub struct RichTarget<'a> {
    pub node: &'a mut NodeTarget,
    pub expected_hash: Option<&'a mut Option<String>>,
}

impl EditOp {
    #[must_use]
    pub const fn is_rich(&self) -> bool {
        matches!(
            self,
            Self::SetMarks(_)
                | Self::SetBlockType(_)
                | Self::InsertTable(_)
                | Self::TableInsertRow(_)
                | Self::TableInsertCol(_)
                | Self::TableDelete(_)
                | Self::InsertImage(_)
                | Self::SetParagraph(_)
                | Self::InsertBlock(_)
                | Self::UpdateTable(_)
                | Self::SetSection(_)
                | Self::SetStyle(_)
                | Self::ApplyTheme(_)
        )
    }

    #[must_use]
    pub const fn is_document_level(&self) -> bool {
        matches!(
            self,
            Self::SetSection(_) | Self::SetStyle(_) | Self::ApplyTheme(_)
        )
    }

    pub const fn rich_target(&mut self) -> Option<RichTarget<'_>> {
        let (node, expected_hash) = match self {
            Self::SetMarks(op) => (&mut op.node, Some(&mut op.expected_hash)),
            Self::SetBlockType(op) => (&mut op.node, Some(&mut op.expected_hash)),
            Self::TableInsertRow(op) | Self::TableInsertCol(op) => {
                (&mut op.table, Some(&mut op.expected_hash))
            },
            Self::TableDelete(op) => (&mut op.table, Some(&mut op.expected_hash)),
            Self::InsertTable(op) => (&mut op.after, None),
            Self::InsertImage(op) => (&mut op.after, None),
            Self::SetParagraph(op) => (&mut op.node, Some(&mut op.expected_hash)),
            Self::UpdateTable(op) => (&mut op.table, Some(&mut op.expected_hash)),
            Self::InsertBlock(op) => (&mut op.after, None),
            _ => return None,
        };
        Some(RichTarget {
            node,
            expected_hash,
        })
    }

    #[must_use]
    pub const fn rich_guard(&self) -> Option<(&NodeTarget, Option<&String>)> {
        Some(match self {
            Self::SetMarks(op) => (&op.node, op.expected_hash.as_ref()),
            Self::SetBlockType(op) => (&op.node, op.expected_hash.as_ref()),
            Self::TableInsertRow(op) | Self::TableInsertCol(op) => {
                (&op.table, op.expected_hash.as_ref())
            },
            Self::TableDelete(op) => (&op.table, op.expected_hash.as_ref()),
            Self::InsertTable(op) => (&op.after, None),
            Self::InsertImage(op) => (&op.after, None),
            Self::SetParagraph(op) => (&op.node, op.expected_hash.as_ref()),
            Self::UpdateTable(op) => (&op.table, op.expected_hash.as_ref()),
            Self::InsertBlock(op) => (&op.after, None),
            _ => return None,
        })
    }
}
