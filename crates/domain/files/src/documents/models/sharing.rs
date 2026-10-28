//! The grant audit log and access requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{DocumentAccessRequestId, DocumentId, TraceId, UserId};
use systemprompt_models::documents::{AccessLevel, AccessRequestStatus, GrantEventKind};
use uuid::Uuid;

use super::grant::GrantTarget;

/// Who made a grant change, recorded on its audit row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantActor {
    pub user: UserId,
    pub trace: TraceId,
}

/// One row of the grant audit log. `actor_user_id` is `None` for a grant
/// the expiry sweep removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentGrantEvent {
    pub id: Uuid,
    pub event: GrantEventKind,
    pub target: GrantTarget,
    pub subject_kind: String,
    pub subject_value: String,
    pub old_level: Option<AccessLevel>,
    pub new_level: Option<AccessLevel>,
    pub valid_until: Option<DateTime<Utc>>,
    pub actor_user_id: Option<UserId>,
    pub trace_id: Option<TraceId>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentAccessRequest {
    pub id: DocumentAccessRequestId,
    pub document_id: DocumentId,
    pub requester_user_id: UserId,
    pub level: AccessLevel,
    pub message: Option<String>,
    pub status: AccessRequestStatus,
    pub decided_by: Option<UserId>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAccessRequest {
    pub document_id: DocumentId,
    pub requester_user_id: UserId,
    pub level: AccessLevel,
    pub message: Option<String>,
}

/// The new owner of a document and the grant the previous owner keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerTransfer {
    pub document_id: DocumentId,
    pub from: UserId,
    pub from_display: String,
    pub to: UserId,
    pub actor: GrantActor,
}
