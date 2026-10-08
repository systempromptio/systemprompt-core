//! Private `query_as!` decode rows for the files tables.
//!
//! `files.id` and `content_files.file_id` are UUID columns, so they decode as
//! [`uuid::Uuid`] and become a [`FileId`] through `FileId::from_uuid` when the
//! row converts into its public model.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::types::Json;
use systemprompt_identifiers::{ContentId, ContextId, FileId, SessionId, TraceId, UserId};

use super::content_file::{ContentFile, FileRole};
use super::file::File;
use super::metadata::FileMetadata;

#[derive(Debug)]
pub(crate) struct FileRow {
    pub id: uuid::Uuid,
    pub path: String,
    pub public_url: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    pub ai_content: bool,
    pub metadata: Json<FileMetadata>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub context_id: Option<ContextId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl From<FileRow> for File {
    fn from(row: FileRow) -> Self {
        Self {
            id: FileId::from_uuid(row.id),
            path: row.path,
            public_url: row.public_url,
            mime_type: row.mime_type,
            size_bytes: row.size_bytes,
            ai_content: row.ai_content,
            metadata: row.metadata,
            user_id: row.user_id,
            session_id: row.session_id,
            trace_id: row.trace_id,
            context_id: row.context_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ContentFileRow {
    pub id: i32,
    pub content_id: ContentId,
    pub file_id: uuid::Uuid,
    pub role: FileRole,
    pub display_order: i32,
    pub created_at: DateTime<Utc>,
}

impl From<ContentFileRow> for ContentFile {
    fn from(row: ContentFileRow) -> Self {
        Self {
            id: row.id,
            content_id: row.content_id,
            file_id: FileId::from_uuid(row.file_id),
            role: row.role,
            display_order: row.display_order,
            created_at: row.created_at,
        }
    }
}
