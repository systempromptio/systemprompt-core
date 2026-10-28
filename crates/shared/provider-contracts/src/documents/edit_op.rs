//! The edit operations an agent or client submits against a projection.
//!
//! Anchored operations (`replace_text`, `delete`, `insert_after`) name their
//! node by id or by quoted text ([`NodeTarget`]). An id target quotes the
//! `hash8` the caller read; a codec compares it with the node's current hash
//! and fails with a stale-anchor error rather than overwrite unseen text. A
//! quote target needs no hash: the quoted text is what the caller read, and
//! [`resolve_targets`](super::resolve_targets) turns it into the id and hash
//! of the matching block before a codec sees the batch.
//!
//! `insert_after` and `replace_text` take their text as plain text or, with
//! `as: markdown`, as Markdown that each codec lowers into native blocks;
//! `append_markdown` adds lowered blocks at the end of the document body.
//!
//! The rich-text and table ops (`set_marks`, `set_block_type`, the table
//! ops, `insert_image`) carry their payload as a struct from `rich_op` and
//! apply through the document's `RichText` model.
//!
//! `restore` and `insert_from_revision` name an earlier revision; the
//! document service expands them (into the revision's bytes, or an
//! `insert_after` of the named node's text) before a codec sees the batch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::num::NonZeroUsize;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DocumentNodeId, DocumentRevisionId};

use super::codec::CodecError;
use super::projection::NodeKind;
use super::rich_op::{InsertImage, InsertTable, SetBlockType, SetMarks, TableDelete, TableInsert};

/// The node an anchored op addresses: a node id from a read, or a quote of
/// the block's text.
///
/// A quote matches the innermost block whose text contains `find`; `nth`
/// (1-based, default 1) picks among matches in document order, counted
/// inside `within` when given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum NodeTarget {
    Id(DocumentNodeId),
    Find(QuoteTarget),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QuoteTarget {
    pub find: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nth: Option<NonZeroUsize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub within: Option<DocumentNodeId>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContentAs {
    #[default]
    Text,
    Markdown,
}

impl ContentAs {
    #[must_use]
    pub const fn is_text(&self) -> bool {
        matches!(self, Self::Text)
    }
}

impl NodeTarget {
    #[must_use]
    pub const fn id(&self) -> Option<&DocumentNodeId> {
        match self {
            Self::Id(id) => Some(id),
            Self::Find(_) => None,
        }
    }

    pub const fn resolved(&self) -> Result<&DocumentNodeId, CodecError> {
        match self {
            Self::Id(id) => Ok(id),
            Self::Find(_) => Err(CodecError::Malformed {
                detail: "a quote target reached the codec unresolved",
            }),
        }
    }
}

impl EditOp {
    pub fn check_resolved(&self) -> Result<(), CodecError> {
        match self {
            Self::ReplaceText {
                content_as: ContentAs::Markdown,
                ..
            } => Err(CodecError::Malformed {
                detail: "replace_text with markdown content reached the codec unresolved",
            }),
            Self::ReplaceText { node, .. }
            | Self::InsertAfter { node, .. }
            | Self::Delete { node, .. } => node.resolved().map(|_| ()),
            Self::Restore { .. } | Self::InsertFromRevision { .. } => Err(CodecError::Malformed {
                detail: "a revision op reached the codec unexpanded",
            }),
            _ => self
                .rich_guard()
                .map_or(Ok(()), |(node, _)| node.resolved().map(|_| ())),
        }
    }
}

pub fn required_hash(hash: Option<&String>) -> Result<&str, CodecError> {
    hash.map(String::as_str).ok_or(CodecError::Malformed {
        detail: "expected_hash is required when node names a node id",
    })
}

const fn paragraph() -> NodeKind {
    NodeKind::Paragraph
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CellValue {
    Bool(bool),
    Number(f64),
    Text(String),
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EditOp {
    ReplaceText {
        node: NodeTarget,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_hash: Option<String>,
        text: String,
        #[serde(rename = "as", default, skip_serializing_if = "ContentAs::is_text")]
        content_as: ContentAs,
    },
    InsertAfter {
        node: NodeTarget,
        #[serde(default = "paragraph")]
        kind: NodeKind,
        text: String,
        #[serde(rename = "as", default, skip_serializing_if = "ContentAs::is_text")]
        content_as: ContentAs,
    },
    Delete {
        node: NodeTarget,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_hash: Option<String>,
    },
    AppendMarkdown {
        markdown: String,
    },
    SetCells {
        sheet: String,
        range: String,
        values: Vec<Vec<CellValue>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_hash: Option<String>,
    },
    InsertSlide {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<DocumentNodeId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        layout: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default)]
        body: Vec<String>,
    },
    StrReplace {
        old_str: String,
        new_str: String,
    },
    SetHtml {
        html: String,
    },
    SetText {
        text: String,
    },
    Restore {
        revision_id: DocumentRevisionId,
    },
    InsertFromRevision {
        revision_id: DocumentRevisionId,
        node: DocumentNodeId,
        after: NodeTarget,
    },
    SetMarks(SetMarks),
    SetBlockType(SetBlockType),
    InsertTable(InsertTable),
    TableInsertRow(TableInsert),
    TableInsertCol(TableInsert),
    TableDelete(TableDelete),
    InsertImage(InsertImage),
}

impl EditOp {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::ReplaceText { .. } => "replace_text",
            Self::InsertAfter { .. } => "insert_after",
            Self::Delete { .. } => "delete",
            Self::SetCells { .. } => "set_cells",
            Self::InsertSlide { .. } => "insert_slide",
            Self::StrReplace { .. } => "str_replace",
            Self::SetHtml { .. } => "set_html",
            Self::SetText { .. } => "set_text",
            Self::AppendMarkdown { .. } => "append_markdown",
            Self::Restore { .. } => "restore",
            Self::InsertFromRevision { .. } => "insert_from_revision",
            Self::SetMarks(_) => "set_marks",
            Self::SetBlockType(_) => "set_block_type",
            Self::InsertTable(_) => "insert_table",
            Self::TableInsertRow(_) => "table_insert_row",
            Self::TableInsertCol(_) => "table_insert_col",
            Self::TableDelete(_) => "table_delete",
            Self::InsertImage(_) => "insert_image",
        }
    }

    #[must_use]
    pub const fn touches(&self) -> Option<&DocumentNodeId> {
        match self {
            Self::ReplaceText { node, .. }
            | Self::InsertAfter { node, .. }
            | Self::Delete { node, .. } => node.id(),
            Self::InsertFromRevision { after, .. } => after.id(),
            Self::InsertSlide { after, .. } => after.as_ref(),
            Self::SetMarks(_)
            | Self::SetBlockType(_)
            | Self::InsertTable(_)
            | Self::TableInsertRow(_)
            | Self::TableInsertCol(_)
            | Self::TableDelete(_)
            | Self::InsertImage(_) => match self.rich_guard() {
                Some((node, _)) => node.id(),
                None => None,
            },
            Self::SetCells { .. }
            | Self::StrReplace { .. }
            | Self::SetHtml { .. }
            | Self::SetText { .. }
            | Self::AppendMarkdown { .. }
            | Self::Restore { .. } => None,
        }
    }
}
