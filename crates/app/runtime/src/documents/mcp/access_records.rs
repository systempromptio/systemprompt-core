//! What `documents_access` and the access routes return: grants, inherited
//! grants with the folder they come from, the grant audit log and open
//! access requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::Serialize;
use systemprompt_files::documents::models::{
    DocumentAccessRequest, DocumentGrant, DocumentGrantEvent, GrantTarget,
};
use systemprompt_files::documents::service::{DocumentAccess, FolderAccess, InheritedGrant};
use systemprompt_identifiers::{
    DocumentAccessRequestId, DocumentFolderId, DocumentId, TraceId, UserId,
};
use systemprompt_mcp::McpOutputSchema;
use systemprompt_models::documents::{AccessLevel, AccessRequestStatus, GrantEventKind};

use super::outputs::documents_output;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct InheritedFrom {
    pub folder_id: DocumentFolderId,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GrantItem {
    pub grant_id: String,
    pub subject_kind: String,
    pub subject_value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub level: AccessLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub expired: bool,
    pub granted_by: UserId,
    pub granted_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<DocumentFolderId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inherited_from: Option<InheritedFrom>,
}

impl From<DocumentGrant> for GrantItem {
    fn from(grant: DocumentGrant) -> Self {
        Self {
            expired: grant.is_expired(Utc::now()),
            grant_id: grant.id.to_string(),
            folder_id: match grant.target {
                GrantTarget::Folder(folder) => Some(folder),
                GrantTarget::Document(_) => None,
            },
            display_name: Some(
                grant
                    .subject_display
                    .unwrap_or_else(|| grant.subject_value.clone()),
            ),
            subject_kind: grant.subject_kind,
            subject_value: grant.subject_value,
            level: grant.access_level,
            valid_until: grant.valid_until,
            granted_by: grant.granted_by,
            granted_at: grant.created_at,
            inherited_from: None,
        }
    }
}

impl From<InheritedGrant> for GrantItem {
    fn from(inherited: InheritedGrant) -> Self {
        Self {
            inherited_from: Some(InheritedFrom {
                folder_id: inherited.folder_id,
                name: inherited.folder_name,
            }),
            ..Self::from(inherited.grant)
        }
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GrantEventItem {
    pub event: GrantEventKind,
    pub subject_kind: String,
    pub subject_value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_level: Option<AccessLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_level: Option<AccessLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_user_id: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<TraceId>,
    pub at: DateTime<Utc>,
}

impl From<DocumentGrantEvent> for GrantEventItem {
    fn from(event: DocumentGrantEvent) -> Self {
        Self {
            event: event.event,
            subject_kind: event.subject_kind,
            subject_value: event.subject_value,
            old_level: event.old_level,
            new_level: event.new_level,
            valid_until: event.valid_until,
            actor_user_id: event.actor_user_id,
            trace_id: event.trace_id,
            at: event.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AccessRequestItem {
    pub request_id: DocumentAccessRequestId,
    pub document_id: DocumentId,
    pub requester_user_id: UserId,
    pub level: AccessLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub status: AccessRequestStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<DateTime<Utc>>,
    pub requested_at: DateTime<Utc>,
}

impl From<DocumentAccessRequest> for AccessRequestItem {
    fn from(request: DocumentAccessRequest) -> Self {
        Self {
            request_id: request.id,
            document_id: request.document_id,
            requester_user_id: request.requester_user_id,
            level: request.level,
            message: request.message,
            status: request.status,
            decided_by: request.decided_by,
            decided_at: request.decided_at,
            requested_at: request.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AccessOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<DocumentId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<DocumentFolderId>,
    pub owner: UserId,
    pub my_access: AccessLevel,
    pub grants: Vec<GrantItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub inherited: Vec<GrantItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<GrantEventItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub requests: Vec<AccessRequestItem>,
}
documents_output!(AccessOutput, "documents_access");

impl From<DocumentAccess> for AccessOutput {
    fn from(access: DocumentAccess) -> Self {
        Self {
            document_id: Some(access.target.id),
            folder_id: None,
            owner: access.owner,
            my_access: access.my_access,
            grants: access.grants.into_iter().map(GrantItem::from).collect(),
            inherited: access.inherited.into_iter().map(GrantItem::from).collect(),
            history: access
                .history
                .into_iter()
                .map(GrantEventItem::from)
                .collect(),
            requests: access
                .requests
                .into_iter()
                .map(AccessRequestItem::from)
                .collect(),
        }
    }
}

impl From<FolderAccess> for AccessOutput {
    fn from(access: FolderAccess) -> Self {
        Self {
            document_id: None,
            folder_id: Some(access.target.id),
            owner: access.owner,
            my_access: access.my_access,
            grants: access.grants.into_iter().map(GrantItem::from).collect(),
            inherited: access.inherited.into_iter().map(GrantItem::from).collect(),
            history: access
                .history
                .into_iter()
                .map(GrantEventItem::from)
                .collect(),
            requests: Vec::new(),
        }
    }
}
