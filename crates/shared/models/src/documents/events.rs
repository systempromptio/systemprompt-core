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
    ContentHash, DocumentAccessRequestId, DocumentCommentId, DocumentEditSessionId, DocumentId,
    DocumentNodeId, DocumentProposalId, DocumentRevisionId, DriveDeviceId, UserId,
};

use super::access::{AccessLevel, ActorKind, ProposalStatus, RevisionOriginKind};
use super::awareness::DocumentAwarenessEvent;
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
    #[serde(rename = "document.proposal.opened")]
    ProposalOpened,
    #[serde(rename = "document.proposal.accepted")]
    ProposalAccepted,
    #[serde(rename = "document.proposal.rejected")]
    ProposalRejected,
    #[serde(rename = "document.proposal.superseded")]
    ProposalSuperseded,
    #[serde(rename = "document.awareness")]
    Awareness,
    #[serde(rename = "document.access_requested")]
    AccessRequested,
}

impl DocumentEventName {
    pub const ALL: [Self; 15] = [
        Self::RevisionCreated,
        Self::Presence,
        Self::Lock,
        Self::Conflict,
        Self::CollabUpdate,
        Self::CommentCreated,
        Self::CommentResolved,
        Self::Shared,
        Self::OwnershipTransferred,
        Self::ProposalOpened,
        Self::ProposalAccepted,
        Self::ProposalRejected,
        Self::ProposalSuperseded,
        Self::Awareness,
        Self::AccessRequested,
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
            Self::ProposalOpened => DocumentProposalEvent::OPENED,
            Self::ProposalAccepted => DocumentProposalEvent::ACCEPTED,
            Self::ProposalRejected => DocumentProposalEvent::REJECTED,
            Self::ProposalSuperseded => DocumentProposalEvent::SUPERSEDED,
            Self::Awareness => DocumentAwarenessEvent::NAME,
            Self::AccessRequested => DocumentAccessRequestedEvent::NAME,
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
#[expect(
    clippy::struct_field_names,
    reason = "the document event wire vocabulary names every reference document_id, comment_id, \
              node_id, parent_id and author_id, as the sibling events do"
)]
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
    pub document_name: String,
    pub recipient_user_id: UserId,
    pub subject_kind: String,
    pub subject_value: String,
    pub level: AccessLevel,
    pub by: UserId,
    pub shared_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

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

/// The payload of every `document.proposal.*` event; the event name says
/// which transition the proposal made and `status` repeats it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentProposalEvent {
    pub document_id: DocumentId,
    pub proposal_id: DocumentProposalId,
    pub status: ProposalStatus,
    pub proposer_id: UserId,
    pub base_revision_id: DocumentRevisionId,
    pub proposed_revision_id: DocumentRevisionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer_id: Option<UserId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landed_revision_id: Option<DocumentRevisionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl DocumentProposalEvent {
    pub const OPENED: &'static str = "document.proposal.opened";
    pub const ACCEPTED: &'static str = "document.proposal.accepted";
    pub const REJECTED: &'static str = "document.proposal.rejected";
    pub const SUPERSEDED: &'static str = "document.proposal.superseded";

    #[must_use]
    pub const fn name_of(status: ProposalStatus) -> &'static str {
        match status {
            ProposalStatus::Open => Self::OPENED,
            ProposalStatus::Accepted => Self::ACCEPTED,
            ProposalStatus::Rejected => Self::REJECTED,
            ProposalStatus::Superseded => Self::SUPERSEDED,
        }
    }
}
