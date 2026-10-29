//! The input and output shapes of `documents_upload`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContentHash, DocumentUploadId};
use systemprompt_mcp::McpOutputSchema;

use super::super::outputs::documents_output;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UploadInput {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mime: String,
    #[serde(default)]
    pub upload_id: Option<DocumentUploadId>,
    #[serde(default)]
    pub content_base64: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub sha256: Option<ContentHash>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UploadMode {
    Inline,
    Resumable,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UploadOutput {
    pub mode: UploadMode,
    pub upload_id: DocumentUploadId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob_ref: Option<ContentHash>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deduplicated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parts_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complete_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_header: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}
documents_output!(UploadOutput, "documents_upload");
