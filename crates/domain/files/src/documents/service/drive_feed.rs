//! The bridge Drive change feed: one ordered stream over revisions and the
//! path history.
//!
//! A revision is reported as `Created` or `Updated`; a path history row as
//! `Deleted` (trash), `Created` (restore, with the head revision) or
//! `Renamed` (rename or move). Both sources are paged by the same
//! `(timestamp, id)` key and merged, so one cursor covers every kind. The
//! timestamps are the writing transaction's start time, so a slow commit can
//! land behind a cursor a client already holds. Each request therefore
//! re-reads [`CHANGE_LAG`] before the client's cursor, and clients dedupe
//! changes by cursor. When a full page would not advance past the cursor,
//! the page is read again from the exact cursor so the feed always makes
//! progress.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use chrono::{DateTime, TimeDelta, Utc};
use systemprompt_identifiers::{DocumentFolderId, DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, DriveChange, DriveChangeKind, DriveChanges};
use uuid::Uuid;

use super::DocumentService;
use crate::documents::access::{AccessCheck, AccessTarget};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision, ListCursor, SubjectPairs};
use crate::documents::repository::DocumentPathChange;

pub const CHANGE_LAG: TimeDelta = TimeDelta::seconds(5);
const MAX_CHANGES_PAGE: i64 = 1000;

const fn cursor_key(cursor: ListCursor) -> (DateTime<Utc>, Uuid) {
    (cursor.updated_at, cursor.id)
}

enum FeedRow {
    Revision(Box<DocumentRevision>, ListCursor),
    Path(DocumentPathChange),
}

impl FeedRow {
    const fn cursor(&self) -> ListCursor {
        match self {
            Self::Revision(_, cursor) => *cursor,
            Self::Path(change) => change.cursor,
        }
    }

    const fn document_id(&self) -> &DocumentId {
        match self {
            Self::Revision(revision, _) => &revision.document_id,
            Self::Path(change) => &change.document_id,
        }
    }
}

struct Seen {
    document: Document,
    path: String,
    trashed: bool,
}

struct FeedScope<'a> {
    who: &'a AuthenticatedUser,
    trace: &'a TraceId,
    pairs: SubjectPairs,
    seen: HashMap<DocumentId, Option<Seen>>,
    paths: HashMap<Option<DocumentFolderId>, Option<String>>,
}

impl DocumentService {
    pub async fn drive_changes(
        &self,
        who: &AuthenticatedUser,
        since: Option<ListCursor>,
        limit: i64,
        trace: &TraceId,
    ) -> Result<DriveChanges, DocumentsError> {
        let limit = limit.clamp(1, MAX_CHANGES_PAGE);
        let lagged = since.map(|c| ListCursor::new(c.updated_at - CHANGE_LAG, c.id));
        let mut rows = self.feed_page(lagged, limit).await?;
        let stalled = rows.len() as i64 == limit
            && since.is_some()
            && rows.last().map(|row| cursor_key(row.cursor())) <= since.map(cursor_key);
        if stalled {
            rows = self.feed_page(since, limit).await?;
        }
        let epoch = ListCursor::new(DateTime::UNIX_EPOCH, Uuid::nil());
        let next = rows
            .iter()
            .map(FeedRow::cursor)
            .chain(since)
            .max_by_key(|c| cursor_key(*c))
            .unwrap_or(epoch);
        let mut scope = FeedScope {
            who,
            trace,
            pairs: self.access.subject_pairs(who).await?,
            seen: HashMap::new(),
            paths: HashMap::new(),
        };
        let mut changes = Vec::new();
        for row in rows {
            if let Some(change) = self.feed_change(&mut scope, row).await? {
                changes.push(change);
            }
        }
        Ok(DriveChanges {
            changes,
            next_cursor: next.to_string(),
        })
    }

    async fn feed_page(
        &self,
        cursor: Option<ListCursor>,
        limit: i64,
    ) -> Result<Vec<FeedRow>, DocumentsError> {
        let revisions = self.repos.revisions.changes_since(cursor, limit).await?;
        let moves = self
            .repos
            .documents
            .path_changes_since(cursor, limit)
            .await?;
        let mut rows: Vec<FeedRow> = revisions
            .into_iter()
            .filter_map(|revision| {
                let id = revision.id.to_uuid().ok()?;
                let cursor = ListCursor::new(revision.created_at, id);
                Some(FeedRow::Revision(Box::new(revision), cursor))
            })
            .chain(moves.into_iter().map(FeedRow::Path))
            .collect();
        rows.sort_by_key(|row| cursor_key(row.cursor()));
        rows.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        Ok(rows)
    }

    async fn feed_change(
        &self,
        scope: &mut FeedScope<'_>,
        row: FeedRow,
    ) -> Result<Option<DriveChange>, DocumentsError> {
        let document_id = row.document_id().clone();
        if !scope.seen.contains_key(&document_id) {
            let seen = self.feed_visible(scope, &document_id).await?;
            scope.seen.insert(document_id.clone(), seen);
        }
        let Some(Some(seen)) = scope.seen.get(&document_id) else {
            return Ok(None);
        };
        let cursor = row.cursor().to_string();
        let change = |path: String, kind, revision: Option<&DocumentRevision>| DriveChange {
            cursor: cursor.clone(),
            document_id: seen.document.id.clone(),
            path,
            revision_id: revision.map(|r| r.id.clone()),
            content_sha256: revision.map(|r| r.content_sha256.clone()),
            kind,
        };
        Ok(match row {
            FeedRow::Revision(_, _) if seen.trashed => None,
            FeedRow::Revision(revision, _) => {
                let kind = if revision.parent_revision_id.is_none() {
                    DriveChangeKind::Created
                } else {
                    DriveChangeKind::Updated
                };
                Some(change(seen.path.clone(), kind, Some(&revision)))
            },
            FeedRow::Path(DocumentPathChange {
                from_path: Some(from),
                to_path: None,
                ..
            }) => Some(change(from, DriveChangeKind::Deleted, None)),
            FeedRow::Path(DocumentPathChange {
                from_path: None,
                to_path: Some(to),
                ..
            }) if !seen.trashed => {
                let head = self.repos.revisions.head(&document_id).await?;
                head.map(|head| change(to, DriveChangeKind::Created, Some(&head)))
            },
            FeedRow::Path(DocumentPathChange {
                from_path: Some(from),
                to_path: Some(to),
                ..
            }) => Some(change(to, DriveChangeKind::Renamed { from }, None)),
            FeedRow::Path(_) => None,
        })
    }

    async fn feed_visible(
        &self,
        scope: &mut FeedScope<'_>,
        document_id: &DocumentId,
    ) -> Result<Option<Seen>, DocumentsError> {
        let mut trashed = false;
        let granted = loop {
            let target = if trashed {
                AccessTarget::Trashed(document_id.clone())
            } else {
                AccessTarget::Document(document_id.clone())
            };
            let check = AccessCheck {
                who: scope.who,
                target: &target,
                action: DocumentAction::View,
                trace: scope.trace,
            };
            match self.access.probe_with(check, &scope.pairs).await {
                Ok(granted) => break granted,
                Err(DocumentsError::Trashed { .. }) if !trashed => trashed = true,
                Err(DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }) => {
                    return Ok(None);
                },
                Err(other) => return Err(other),
            }
        };
        let Some(document) = granted.document else {
            return Ok(None);
        };
        let path = self.cached_path(&document, &mut scope.paths).await?;
        Ok(Some(Seen {
            document,
            path,
            trashed,
        }))
    }
}
