//! Serializable output shapes for the `core content files` commands: the
//! files linked to a content item, the content items a file is linked to,
//! link and unlink results, and the featured image.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_cli::kernel::FileSummary;
use systemprompt_identifiers::{ContentId, FileId};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentFilesOutput {
    pub content_id: ContentId,
    pub files: Vec<ContentFileRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentFileRow {
    pub file_id: FileId,
    pub path: String,
    pub mime_type: String,
    pub role: String,
    pub display_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentLinkOutput {
    pub file_id: FileId,
    pub content_id: ContentId,
    pub role: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentUnlinkOutput {
    pub file_id: FileId,
    pub content_id: ContentId,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FeaturedImageOutput {
    pub content_id: ContentId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<FileSummary>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileContentLinksOutput {
    pub file_id: FileId,
    pub links: Vec<FileContentLinkRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileContentLinkRow {
    pub content_id: ContentId,
    pub role: String,
    pub display_order: i32,
    pub created_at: DateTime<Utc>,
}
