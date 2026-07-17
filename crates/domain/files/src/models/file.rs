//! Persisted file row model.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::Json;
use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};

use super::metadata::FileMetadata;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct File {
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

impl File {
    pub fn id(&self) -> FileId {
        FileId::new(self.id.to_string())
    }
}
