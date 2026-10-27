//! JSON views of the document store's rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::Serialize;
use systemprompt_files::documents::models::{
    Document, DocumentFolder, DocumentGrant, DocumentLock, DocumentPresenceRow, DocumentRevision,
    GrantTarget, ListedDocument,
};
use systemprompt_files::documents::service::{DocumentView, Listing};
use systemprompt_identifiers::{
    ContentHash, DocumentFolderId, DocumentId, DocumentRevisionId, DocumentTemplateId, UserId,
};
use systemprompt_models::documents::{
    AccessLevel, ActorKind, AgentEditPolicy, DocumentFormat, PresenceSource, PresenceState,
    RevisionOriginKind,
};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub(super) struct DocumentJson {
    id: DocumentId,
    folder_id: Option<DocumentFolderId>,
    name: String,
    format: DocumentFormat,
    mime_type: String,
    head_revision_id: Option<DocumentRevisionId>,
    template_id: Option<DocumentTemplateId>,
    owner_user_id: UserId,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<Document> for DocumentJson {
    fn from(d: Document) -> Self {
        Self {
            id: d.id,
            folder_id: d.folder_id,
            name: d.name,
            format: d.format,
            mime_type: d.mime_type,
            head_revision_id: d.head_revision_id,
            template_id: d.template_id,
            owner_user_id: d.owner_user_id,
            created_at: d.created_at,
            updated_at: d.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct RevisionJson {
    id: DocumentRevisionId,
    document_id: DocumentId,
    parent_revision_id: Option<DocumentRevisionId>,
    content_sha256: ContentHash,
    author_user_id: UserId,
    actor_kind: ActorKind,
    origin_kind: RevisionOriginKind,
    message: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<DocumentRevision> for RevisionJson {
    fn from(r: DocumentRevision) -> Self {
        Self {
            id: r.id,
            document_id: r.document_id,
            parent_revision_id: r.parent_revision_id,
            content_sha256: r.content_sha256,
            author_user_id: r.author_user_id,
            actor_kind: r.actor_kind,
            origin_kind: r.origin_kind,
            message: r.message,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct DocumentDetail {
    document: DocumentJson,
    head: Option<RevisionJson>,
    my_access: AccessLevel,
    folder_path: Option<String>,
}

impl From<DocumentView> for DocumentDetail {
    fn from(v: DocumentView) -> Self {
        Self {
            document: v.document.into(),
            head: v.head.map(Into::into),
            my_access: v.my_access,
            folder_path: v.folder_path,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct CreatedJson {
    pub(super) document: DocumentJson,
    pub(super) revision: RevisionJson,
}

#[derive(Debug, Serialize)]
pub(super) struct FolderJson {
    id: DocumentFolderId,
    parent_id: Option<DocumentFolderId>,
    name: String,
    owner_user_id: UserId,
    agent_edit_policy: AgentEditPolicy,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<DocumentFolder> for FolderJson {
    fn from(f: DocumentFolder) -> Self {
        Self {
            id: f.id,
            parent_id: f.parent_id,
            name: f.name,
            owner_user_id: f.owner_user_id,
            agent_edit_policy: f.agent_edit_policy,
            created_at: f.created_at,
            updated_at: f.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct ListedJson {
    #[serde(flatten)]
    document: DocumentJson,
    size_bytes: Option<i64>,
    my_access: AccessLevel,
}

impl From<ListedDocument> for ListedJson {
    fn from(l: ListedDocument) -> Self {
        Self {
            document: l.document.into(),
            size_bytes: l.size_bytes,
            my_access: l.my_access,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct ListingJson {
    folders: Vec<FolderJson>,
    documents: Vec<ListedJson>,
    next_cursor: Option<String>,
}

impl From<Listing> for ListingJson {
    fn from(l: Listing) -> Self {
        Self {
            folders: l.folders.into_iter().map(Into::into).collect(),
            documents: l.documents.into_iter().map(Into::into).collect(),
            next_cursor: l.next_cursor.map(|c| c.to_string()),
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct GrantJson {
    id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    document_id: Option<DocumentId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    folder_id: Option<DocumentFolderId>,
    subject_kind: String,
    subject_value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    access_level: AccessLevel,
    granted_by: UserId,
    valid_until: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl From<DocumentGrant> for GrantJson {
    fn from(g: DocumentGrant) -> Self {
        let (document_id, folder_id) = match g.target {
            GrantTarget::Document(id) => (Some(id), None),
            GrantTarget::Folder(id) => (None, Some(id)),
        };
        Self {
            id: g.id,
            document_id,
            folder_id,
            subject_kind: g.subject_kind,
            subject_value: g.subject_value,
            display_name: g.subject_display,
            access_level: g.access_level,
            granted_by: g.granted_by,
            valid_until: g.valid_until,
            created_at: g.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct PresenceJson {
    user_id: UserId,
    source: PresenceSource,
    state: PresenceState,
    // JSON: editor cursor position; the shape belongs to the client's editor.
    cursor: Option<serde_json::Value>,
    updated_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl From<DocumentPresenceRow> for PresenceJson {
    fn from(p: DocumentPresenceRow) -> Self {
        Self {
            user_id: p.user_id,
            source: p.source,
            state: p.state,
            cursor: p.cursor,
            updated_at: p.updated_at,
            expires_at: p.expires_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct LockJson {
    document_id: DocumentId,
    holder_user_id: UserId,
    holder_kind: &'static str,
    holder_id: String,
    acquired_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl From<DocumentLock> for LockJson {
    fn from(l: DocumentLock) -> Self {
        Self {
            document_id: l.document_id,
            holder_user_id: l.holder_user_id,
            holder_kind: l.holder_kind.as_str(),
            holder_id: l.holder_id,
            acquired_at: l.acquired_at,
            expires_at: l.expires_at,
        }
    }
}
