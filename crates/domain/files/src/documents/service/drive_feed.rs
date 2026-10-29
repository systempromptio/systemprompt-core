//! The bridge Drive change feed: one ordered stream over revisions and the
//! path history.
//!
//! A revision is reported as `Created` or `Updated`; a path history row as
//! `Deleted` (trash), `Created` (restore, with the head revision) or
//! `Renamed` (rename or move). Both sources are paged by the same landing
//! position under one horizon and merged, so one cursor covers every kind
//! and a change never lands behind a cursor already handed out.
//!
//! A path history row that names a grant subject marks where that subject's
//! access changed, and is paged only for a caller that subject covers. A
//! `lost` row is reported as `Deleted` at its recorded path when the document
//! is no longer visible to the caller; a `gained` row is reported as
//! `Created` at the current path with the head revision while it is visible.
//! A caller whose view did not change (they still see a lost document through
//! another grant, or a gained one is gone again) gets nothing for it, so a
//! grant revoked before the reader caught up leaves only the `Deleted`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::{DocumentFolderId, DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, DriveChange, DriveChangeKind, DriveChanges};

use super::DocumentService;
use crate::documents::access::{AccessCheck, AccessTarget};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision, DriveCursor, SubjectPairs};
use crate::documents::repository::{DocumentPathChange, SubjectAccess};

const MAX_CHANGES_PAGE: i64 = 1000;

enum FeedRow {
    Revision(Box<DocumentRevision>, DriveCursor),
    Path(DocumentPathChange),
}

impl FeedRow {
    const fn cursor(&self) -> DriveCursor {
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
        since: Option<DriveCursor>,
        limit: i64,
        trace: &TraceId,
    ) -> Result<DriveChanges, DocumentsError> {
        let limit = limit.clamp(1, MAX_CHANGES_PAGE);
        let since = since.unwrap_or_default();
        let pairs = self.access.subject_pairs(who).await?;
        let rows = self.feed_page(since, &pairs, limit).await?;
        let next = rows.last().map_or(since, FeedRow::cursor);
        let mut scope = FeedScope {
            who,
            trace,
            pairs,
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
        since: DriveCursor,
        pairs: &SubjectPairs,
        limit: i64,
    ) -> Result<Vec<FeedRow>, DocumentsError> {
        let horizon = self.repos.revisions.feed_horizon().await?;
        let revisions = self
            .repos
            .revisions
            .landed_since(since, horizon, limit)
            .await?;
        let moves = self
            .repos
            .documents
            .path_changes_since(since, horizon, pairs, limit)
            .await?;
        let mut rows: Vec<FeedRow> = revisions
            .into_iter()
            .map(|(revision, cursor)| FeedRow::Revision(Box::new(revision), cursor))
            .chain(moves.into_iter().map(FeedRow::Path))
            .collect();
        rows.sort_by_key(FeedRow::cursor);
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
        let cursor = row.cursor().to_string();
        if let FeedRow::Path(
            change @ DocumentPathChange {
                access: Some(_), ..
            },
        ) = &row
        {
            return self.subject_change(scope, change, cursor).await;
        }
        let Some(Some(seen)) = scope.seen.get(&document_id) else {
            return Ok(None);
        };
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

    async fn subject_change(
        &self,
        scope: &FeedScope<'_>,
        change: &DocumentPathChange,
        cursor: String,
    ) -> Result<Option<DriveChange>, DocumentsError> {
        let document_id = &change.document_id;
        let seen = scope.seen.get(document_id).and_then(Option::as_ref);
        Ok(match (change.access, seen) {
            (Some(SubjectAccess::Lost), None) => {
                change.from_path.as_ref().map(|path| DriveChange {
                    cursor,
                    document_id: document_id.clone(),
                    path: path.to_owned(),
                    revision_id: None,
                    content_sha256: None,
                    kind: DriveChangeKind::Deleted,
                })
            },
            (Some(SubjectAccess::Gained), Some(seen)) if !seen.trashed => self
                .repos
                .revisions
                .head(document_id)
                .await?
                .map(|head| DriveChange {
                    cursor,
                    document_id: document_id.clone(),
                    path: seen.path.clone(),
                    revision_id: Some(head.id),
                    content_sha256: Some(head.content_sha256),
                    kind: DriveChangeKind::Created,
                }),
            _ => None,
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
