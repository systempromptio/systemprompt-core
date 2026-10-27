//! Scoped listings and the hook filter applied to every document listing.
//!
//! A `folder` listing authorizes the folder and lists its visible subfolders
//! on the first page; `shared`, `mine` and `recent` span every folder. The
//! repository decides grant visibility, and every row then passes the authz
//! hook for its folder, so a listing never shows what a read would refuse.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::{DocumentFolderId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentAction;

use super::{DocumentService, ListFolderRequest, Listing};
use crate::documents::access::{AccessCheck, AccessTarget};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentFolder, ListScope, ScopedListing, SubjectPairs};

fn name_matches(name: &str, query: Option<&str>) -> bool {
    query
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .is_none_or(|q| name.to_lowercase().contains(&q.to_lowercase()))
}

impl DocumentService {
    pub async fn list_folder(
        &self,
        who: &AuthenticatedUser,
        request: ListFolderRequest,
        trace: &TraceId,
    ) -> Result<Listing, DocumentsError> {
        let (pairs, folders) = if request.scope == ListScope::Folder {
            let granted = self
                .access
                .authorize(
                    who,
                    AccessTarget::Folder(request.folder.clone()),
                    DocumentAction::View,
                    trace,
                )
                .await?;
            let folders = if request.cursor.is_none() {
                self.visible_children(who, &request, &granted.pairs, trace)
                    .await?
            } else {
                Vec::new()
            };
            (granted.pairs, folders)
        } else {
            (self.access.subject_pairs(who).await?, Vec::new())
        };
        let listing = ScopedListing {
            scope: request.scope,
            folder_id: request.folder,
            query: request.query,
            sort: request.sort,
            cursor: request.cursor,
            limit: request.limit,
        };
        let (page, next_cursor) = self
            .repos
            .documents
            .list_scoped(&listing, &pairs, &who.id)
            .await?;
        let documents = self
            .hook_filter(who, page, |listed| listed.document.folder_id.clone(), trace)
            .await;
        Ok(Listing {
            folders,
            documents,
            next_cursor,
        })
    }

    async fn visible_children(
        &self,
        who: &AuthenticatedUser,
        request: &ListFolderRequest,
        pairs: &SubjectPairs,
        trace: &TraceId,
    ) -> Result<Vec<DocumentFolder>, DocumentsError> {
        let mut folders = Vec::new();
        for child in self
            .repos
            .folders
            .list_children(request.folder.as_ref())
            .await?
        {
            if !name_matches(&child.name, request.query.as_deref()) {
                continue;
            }
            let target = AccessTarget::Folder(Some(child.id.clone()));
            let check = AccessCheck {
                who,
                target: &target,
                action: DocumentAction::View,
                trace,
            };
            match self.access.probe_with(check, pairs).await {
                Ok(_) => folders.push(child),
                Err(DocumentsError::Forbidden { .. }) => {},
                Err(other) => return Err(other),
            }
        }
        Ok(folders)
    }

    pub(super) async fn hook_visible(
        &self,
        who: &AuthenticatedUser,
        documents: Vec<Document>,
        trace: &TraceId,
    ) -> Vec<Document> {
        self.hook_filter(who, documents, |d| d.folder_id.clone(), trace)
            .await
    }

    async fn hook_filter<T>(
        &self,
        who: &AuthenticatedUser,
        items: Vec<T>,
        folder_of: impl Fn(&T) -> Option<DocumentFolderId>,
        trace: &TraceId,
    ) -> Vec<T> {
        let mut allowed: HashMap<Option<DocumentFolderId>, bool> = HashMap::new();
        let mut visible = Vec::with_capacity(items.len());
        for item in items {
            let key = folder_of(&item);
            let admit = if let Some(admit) = allowed.get(&key) {
                *admit
            } else {
                let admit = self.access.probe_folder(who, key.as_ref(), trace).await;
                allowed.insert(key, admit);
                admit
            };
            if admit {
                visible.push(item);
            }
        }
        visible
    }
}
