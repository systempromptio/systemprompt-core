//! The bridge Drive feed: manifest, change feed and commits by path.
//!
//! The change feed itself is in `drive_feed`. The manifest pages the caller's
//! own documents, then the documents shared with them, through the scoped
//! listing statements on the authorization pool.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use chrono::Utc;
use systemprompt_identifiers::{DocumentFolderId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DriveEntry, DriveManifest};

use super::DocumentService;
use super::support::join_path;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DriveCursor, ListScope, ListSort, ScopedListing};

const MANIFEST_PAGE: i64 = 500;

pub fn parse_drive_cursor(cursor: &str) -> Result<DriveCursor, DocumentsError> {
    cursor
        .parse()
        .map_err(|source| DocumentsError::CursorReset {
            cursor: cursor.to_owned(),
            source,
        })
}

impl DocumentService {
    pub async fn drive_manifest(
        &self,
        who: &AuthenticatedUser,
        trace: &TraceId,
    ) -> Result<DriveManifest, DocumentsError> {
        let generated_at = Utc::now();
        let horizon = self.repos.revisions.feed_horizon().await?;
        let pairs = self.access.subject_pairs(who).await?;
        let mut paths = HashMap::new();
        let mut entries = Vec::new();
        for scope in [ListScope::Mine, ListScope::Shared] {
            let mut cursor = None;
            loop {
                let listing = ScopedListing {
                    scope,
                    folder_id: None,
                    query: None,
                    sort: ListSort::Updated,
                    cursor,
                    limit: MANIFEST_PAGE,
                };
                let (page, next) = self
                    .repos
                    .documents
                    .list_scoped(&listing, &pairs, &who.id)
                    .await?;
                let documents = page.into_iter().map(|listed| listed.document).collect();
                for document in self.hook_visible(who, documents, trace).await {
                    if let Some(entry) = self.drive_entry(&document, &mut paths).await? {
                        entries.push(entry);
                    }
                }
                match next {
                    Some(next) => cursor = Some(next),
                    None => break,
                }
            }
        }
        Ok(DriveManifest {
            generated_at,
            cursor: DriveCursor {
                xid: horizon.0,
                seq: 0,
            }
            .to_string(),
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

    pub(super) async fn cached_path(
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
}
