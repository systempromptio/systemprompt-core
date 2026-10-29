//! The `document_grants` row shape the grant queries select into.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{DocumentFolderId, DocumentId, UserId};
use systemprompt_models::documents::AccessLevel;
use systemprompt_traits::RepositoryError;
use uuid::Uuid;

use super::rows::{RepoResult, parse};
use crate::documents::models::{DocumentGrant, GrantTarget};

pub(super) struct GrantRow {
    pub(super) id: Uuid,
    pub(super) document_id: Option<DocumentId>,
    pub(super) folder_id: Option<DocumentFolderId>,
    pub(super) subject_kind: String,
    pub(super) subject_value: String,
    pub(super) access_level: String,
    pub(super) granted_by: UserId,
    pub(super) valid_until: Option<DateTime<Utc>>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) subject_display: Option<String>,
}

impl TryFrom<GrantRow> for DocumentGrant {
    type Error = RepositoryError;

    fn try_from(row: GrantRow) -> RepoResult<Self> {
        let target = match (row.document_id, row.folder_id) {
            (Some(document), None) => GrantTarget::Document(document),
            (None, Some(folder)) => GrantTarget::Folder(folder),
            _ => {
                return Err(RepositoryError::invalid_data(
                    "document_grants.target",
                    "exactly one of document_id and folder_id must be set",
                ));
            },
        };
        Ok(Self {
            access_level: parse("document_grants.access_level", &row.access_level)?,
            id: row.id,
            target,
            subject_kind: row.subject_kind,
            subject_value: row.subject_value,
            granted_by: row.granted_by,
            valid_until: row.valid_until,
            created_at: row.created_at,
            updated_at: row.updated_at,
            subject_display: row.subject_display,
        })
    }
}

pub(super) const fn level_from_rank(rank: i32) -> Option<AccessLevel> {
    match rank {
        1 => Some(AccessLevel::View),
        2 => Some(AccessLevel::Comment),
        3 => Some(AccessLevel::Edit),
        4 => Some(AccessLevel::Share),
        5 => Some(AccessLevel::Own),
        _ => None,
    }
}
