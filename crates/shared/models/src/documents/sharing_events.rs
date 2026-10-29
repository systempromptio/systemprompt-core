//! Payloads of the sharing events.
//!
//! A document shared with someone (by a grant or by a mention), an access
//! request, and an ownership transfer each name their recipient and reach
//! only them, never the document's live topic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DocumentAccessRequestId, DocumentCommentId, DocumentId, UserId};

use super::access::AccessLevel;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentSharedEvent {
    pub document_id: DocumentId,
    pub document_name: String,
    pub recipient_user_id: UserId,
    pub subject_kind: String,
    pub subject_value: String,
    pub level: AccessLevel,
    pub by: UserId,
    pub shared_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default)]
    pub v: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment_id: Option<DocumentCommentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

pub const SHARED_EVENT_VERSION: u32 = 1;

impl DocumentSharedEvent {
    pub const NAME: &'static str = "document.shared";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentAccessRequestedEvent {
    pub document_id: DocumentId,
    pub document_name: String,
    pub request_id: DocumentAccessRequestId,
    pub requester_user_id: UserId,
    pub requester_display: String,
    pub level: AccessLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl DocumentAccessRequestedEvent {
    pub const NAME: &'static str = "document.access_requested";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentOwnershipTransferredEvent {
    pub document_id: DocumentId,
    pub from: UserId,
    pub to: UserId,
}

impl DocumentOwnershipTransferredEvent {
    pub const NAME: &'static str = "document.ownership_transferred";
}
