//! Outputs of the lifecycle tools, and the output contract of the comment
//! and access views.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::Serialize;
use systemprompt_mcp::McpOutputSchema;

use super::outputs::{DocumentItem, documents_output};
use crate::documents::views::{AccessOutput, CommentOutput};

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DocumentAck {
    #[serde(flatten)]
    pub document: DocumentItem,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub trashed: bool,
}
documents_output!(DocumentAck, "documents_document");
documents_output!(CommentOutput, "documents_comment");
documents_output!(AccessOutput, "documents_access");
