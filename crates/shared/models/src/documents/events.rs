//! Payloads of the AG-UI custom events the document store emits.
//!
//! Each payload carries its event name as `NAME`; [`DocumentEventName`]
//! enumerates them for routing and subscription filters.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    ContentHash, DocumentCommentId, DocumentEditSessionId, DocumentId, DocumentNodeId,
    DocumentRevisionId, DriveDeviceId, UserId,
};

use super::access::{AccessLevel, ActorKind, RevisionOriginKind};
use super::drive::{PresenceSource, PresenceState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum DocumentEventName {
    #[serde(rename = "document.revision.created")]
    RevisionCreated,
    #[serde(rename = "document.presence")]
    Presence,
    #[serde(rename = "document.lock")]
    Lock,
    #[serde(rename = "document.conflict")]
    Conflict,
    #[serde(rename = "document.collab.update")]
    CollabUpdate,
    #[serde(rename = "document.comment.created")]
    CommentCreated,
    #[serde(rename = "document.comment.resolved")]
    CommentResolved,
    #[serde(rename = "document.shared")]
    Shared,
    #[serde(rename = "document.ownership_transferred")]
    OwnershipTransferred,
}

impl DocumentEventName {
    pub const ALL: [Self; 9] = [
        Self::RevisionCreated,
        Self::Presence,
        Self::Lock,
        Self::Conflict,
        Self::CollabUpdate,
        Self::CommentCreated,
        Self::CommentResolved,
        Self::Shared,
        Self::OwnershipTransferred,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RevisionCreated => DocumentRevisionCreatedEvent::NAME,
            Self::Presence => DocumentPresenceEvent::NAME,
            Self::Lock => DocumentLockEvent::NAME,
            Self::Conflict => DocumentConflictEvent::NAME,
            Self::CollabUpdate => DocumentCollabUpdateEvent::NAME,
            Self::CommentCreated => DocumentCommentCreatedEvent::NAME,
            Self::CommentResolved => DocumentCommentResolvedEvent::NAME,
            Self::Shared => DocumentSharedEvent::NAME,
            Self::OwnershipTransferred => DocumentOwnershipTransferredEvent::NAME,
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|event| event.as_str() == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentRevisionCreatedEvent {
    pub document_id: DocumentId,
    pub revision_id: DocumentRevisionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_revision_id: Option<DocumentRevisionId>,
    pub content_sha256: ContentHash,
    pub author_id: UserId,
    pub actor_kind: ActorKind,
    pub origin_kind: RevisionOriginKind,
}

impl DocumentRevisionCreatedEvent {
    pub const NAME: &'static str = "document.revision.created";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentPresenceEvent {
    pub document_id: DocumentId,
    pub user_id: UserId,
    pub state: PresenceState,
    pub source_kind: PresenceSource,
    pub source_ref: String,
    // JSON: editor cursor position; the shape belongs to the client's editor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<serde_json::Value>,
}

impl DocumentPresenceEvent {
    pub const NAME: &'static str = "document.presence";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentLockEvent {
    pub document_id: DocumentId,
    pub locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holder_id: Option<UserId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_session_id: Option<DocumentEditSessionId>,
}

impl DocumentLockEvent {
    pub const NAME: &'static str = "document.lock";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentConflictEvent {
    pub document_id: DocumentId,
    pub path: String,
    pub head_revision_id: DocumentRevisionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision_id: Option<DocumentRevisionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<DriveDeviceId>,
}

impl DocumentConflictEvent {
    pub const NAME: &'static str = "document.conflict";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentCollabUpdateEvent {
    pub document_id: DocumentId,
    pub edit_session_id: DocumentEditSessionId,
    pub seq: i64,
    pub update_b64: String,
    pub author_id: UserId,
}

impl DocumentCollabUpdateEvent {
    pub const NAME: &'static str = "document.collab.update";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentCommentCreatedEvent {
    pub document_id: DocumentId,
    pub comment_id: DocumentCommentId,
    pub node_id: DocumentNodeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<DocumentCommentId>,
    pub author_id: UserId,
}

impl DocumentCommentCreatedEvent {
    pub const NAME: &'static str = "document.comment.created";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentCommentResolvedEvent {
    pub document_id: DocumentId,
    pub comment_id: DocumentCommentId,
    pub resolved_by: UserId,
}

impl DocumentCommentResolvedEvent {
    pub const NAME: &'static str = "document.comment.resolved";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentSharedEvent {
    pub document_id: DocumentId,
    pub subject_kind: String,
    pub subject_value: String,
    pub level: AccessLevel,
    pub by: UserId,
}

impl DocumentSharedEvent {
    pub const NAME: &'static str = "document.shared";
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
