//! The edit operations an agent or client submits against a projection.
//!
//! Anchored operations (`replace_text`, `delete`) quote the `hash8` the
//! caller read; a codec compares it with the node's current hash and fails
//! with a stale-anchor error rather than overwrite unseen text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::DocumentNodeId;

use super::projection::NodeKind;

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
        node: DocumentNodeId,
        expected_hash: String,
        text: String,
    },
    InsertAfter {
        node: DocumentNodeId,
        kind: NodeKind,
        text: String,
    },
    Delete {
        node: DocumentNodeId,
        expected_hash: String,
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
        }
    }

    #[must_use]
    pub const fn touches(&self) -> Option<&DocumentNodeId> {
        match self {
            Self::ReplaceText { node, .. }
            | Self::InsertAfter { node, .. }
            | Self::Delete { node, .. } => Some(node),
            Self::InsertSlide { after, .. } => after.as_ref(),
            Self::SetCells { .. }
            | Self::StrReplace { .. }
            | Self::SetHtml { .. }
            | Self::SetText { .. } => None,
        }
    }
}
