//! The bridge Drive feed: manifest, change feed and commits by path.
//!
//! The change feed pages `document_revisions` by `(created_at, id)`.
//! `created_at` is the inserting transaction's start time, so a slow commit
//! can land behind a cursor a client already holds. Each request therefore
//! re-reads [`CHANGE_LAG`] before the client's cursor, and clients dedupe
//! changes by revision id. When a full page would not advance past the
//! cursor, the page is read again from the exact cursor so the feed always
//! makes progress. The feed carries revisions only: a rename, move or delete
//! reaches the client when it refreshes the manifest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use chrono::{DateTime, TimeDelta, Utc};
use systemprompt_identifiers::{DocumentFolderId, DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    DocumentAction, DriveChange, DriveChangeKind, DriveChanges, DriveEntry, DriveManifest,
};
use uuid::Uuid;

use super::DocumentService;
use super::support::join_path;
use crate::documents::access::{AccessCheck, AccessTarget};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision, ListCursor, VisibleListing};

pub const CHANGE_LAG: TimeDelta = TimeDelta::seconds(5);
const MANIFEST_PAGE: i64 = 500;
const MAX_CHANGES_PAGE: i64 = 1000;

pub fn parse_drive_cursor(cursor: &str) -> Result<ListCursor, DocumentsError> {
    cursor
        .parse()
        .map_err(|source| DocumentsError::InvalidCursor {
            cursor: cursor.to_owned(),
            source,
        })
}

const fn cursor_key(cursor: ListCursor) -> (DateTime<Utc>, Uuid) {
    (cursor.updated_at, cursor.id)
}

fn revision_cursor(revision: &DocumentRevision) -> Option<ListCursor> {
    revision
        .id
        .to_uuid()
        .ok()
        .map(|id| ListCursor::new(revision.created_at, id))
}

impl DocumentService {
    pub async fn drive_manifest(
        &self,
        who: &AuthenticatedUser,
        trace: &TraceId,
    ) -> Result<DriveManifest, DocumentsError> {
        let generated_at = Utc::now();
        let pairs = self.access.subject_pairs(who).await?;
        let mut paths = HashMap::new();
        let mut entries = Vec::new();
        let mut cursor = None;
        loop {
            let listing = VisibleListing {
                folder_id: None,
                query: None,
                cursor,
                limit: MANIFEST_PAGE,
            };
            let (page, next) = self
                .repos
                .documents
                .list_visible(listing, &pairs, &who.id)
                .await?;
            for document in self.hook_visible(who, page, trace).await {
                if let Some(entry) = self.drive_entry(&document, &mut paths).await? {
                    entries.push(entry);
                }
            }
            match next {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        Ok(DriveManifest {
            generated_at,
            cursor: ListCursor::new(generated_at, Uuid::nil()).to_string(),
            entries,
        })
    }

    async fn drive_entry(
        &self,
        document: &Document,
        paths: &mut HashMap<Option<DocumentFolderId>, Option<String>>,
    ) -> Result<Option<DriveEntry>, DocumentsError> {
        let Some(head) = self.repos.revisions.head(&document.id).await? else {
            return Ok(None);
        };
        let Some(blob) = self.repos.blobs.find(&head.content_sha256).await? else {
            tracing::warn!(
                document_id = %document.id,
                content_sha256 = %head.content_sha256,
                "Drive manifest skipped a document whose head blob has no row"
            );
            return Ok(None);
        };
        let path = self.cached_path(document, paths).await?;
        Ok(Some(DriveEntry {
            document_id: document.id.clone(),
            path,
            format: document.format,
            head_revision_id: head.id,
            content_sha256: head.content_sha256,
            size_bytes: blob.size_bytes.unsigned_abs(),
            updated_at: document.updated_at,
        }))
    }

    async fn cached_path(
        &self,
        document: &Document,
        paths: &mut HashMap<Option<DocumentFolderId>, Option<String>>,
    ) -> Result<String, DocumentsError> {
        let key = document.folder_id.clone();
        let folder = if let Some(folder) = paths.get(&key) {
            folder.clone()
        } else {
            let folder = self.folder_path(key.as_ref()).await?;
            paths.insert(key, folder.clone());
            folder
        };
        Ok(join_path(folder.as_deref(), &document.name))
    }

    pub async fn drive_changes(
        &self,
        who: &AuthenticatedUser,
        since: Option<ListCursor>,
        limit: i64,
        trace: &TraceId,
    ) -> Result<DriveChanges, DocumentsError> {
        let limit = limit.clamp(1, MAX_CHANGES_PAGE);
        let lagged = since.map(|c| ListCursor::new(c.updated_at - CHANGE_LAG, c.id));
        let mut rows = self.repos.revisions.changes_since(lagged, limit).await?;
        let stalled = rows.len() as i64 == limit
            && since.is_some()
            && rows.last().and_then(revision_cursor).map(cursor_key) <= since.map(cursor_key);
        if stalled {
            rows = self.repos.revisions.changes_since(since, limit).await?;
        }
        let epoch = ListCursor::new(DateTime::UNIX_EPOCH, Uuid::nil());
        let next = rows
            .iter()
            .filter_map(revision_cursor)
            .chain(since)
            .max_by_key(|c| cursor_key(*c))
            .unwrap_or(epoch);
        let pairs = self.access.subject_pairs(who).await?;
        let mut visible: HashMap<DocumentId, Option<(Document, String)>> = HashMap::new();
        let mut paths = HashMap::new();
        let mut changes = Vec::new();
        for revision in rows {
            if !visible.contains_key(&revision.document_id) {
                let target = AccessTarget::Document(revision.document_id.clone());
                let check = AccessCheck {
                    who,
                    target: &target,
                    action: DocumentAction::View,
                    trace,
                };
                let seen = match self.access.probe_with(check, &pairs).await {
                    Ok(granted) => match granted.document {
                        Some(document) => {
                            let path = self.cached_path(&document, &mut paths).await?;
                            Some((document, path))
                        },
                        None => None,
                    },
                    Err(DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }) => None,
                    Err(other) => return Err(other),
                };
                visible.insert(revision.document_id.clone(), seen);
            }
            let Some(Some((document, path))) = visible.get(&revision.document_id) else {
                continue;
            };
            let Some(cursor) = revision_cursor(&revision) else {
                continue;
            };
            changes.push(DriveChange {
                cursor: cursor.to_string(),
                document_id: document.id.clone(),
                path: path.clone(),
                kind: if revision.parent_revision_id.is_none() {
                    DriveChangeKind::Created
                } else {
                    DriveChangeKind::Updated
                },
                revision_id: Some(revision.id),
                content_sha256: Some(revision.content_sha256),
            });
        }
        Ok(DriveChanges {
            changes,
            next_cursor: next.to_string(),
        })
    }
}
