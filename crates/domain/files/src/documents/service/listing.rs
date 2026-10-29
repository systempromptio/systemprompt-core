//! Scoped listings and the hook filter applied to every document listing.
//!
//! A `folder` listing authorizes the folder and lists its visible subfolders
//! on the first page: one statement per page of children decides grant
//! visibility for all of them, and each visible child then passes the authz
//! hook (and, for an agent caller, the agent gate). Pages advance on the last
//! fetched child, whether or not the hook admitted it, and stop at a fixed
//! page bound; `shared`, `mine` and
//! `recent` span every folder. The repository decides grant visibility, and
//! every row then passes the authz hook for its folder, so a listing never
//! shows what a read would refuse, and a document whose agent cap is `none` is
//! dropped for an agent caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::{DocumentFolderId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentAction;

use super::{DocumentService, ListFolderRequest, Listing};
use crate::documents::access::{AccessCheck, AccessTarget, FolderProbe};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentFolder, ListScope, ScopedListing, SubjectPairs};
use crate::documents::repository::{ChildFolderPage, like_pattern};

const CHILD_FOLDER_PAGE: i64 = 200;
const CHILD_FOLDER_PAGES: usize = 10;

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
            .hook_filter(who, page, |listed| &listed.document, trace)
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
        let pattern = like_pattern(request.query.as_deref());
        let agent = self.access.actor().as_agent().is_some();
        let mut child_ancestors = Vec::new();
        if let Some(parent) = &request.folder {
            child_ancestors.push(parent.clone());
            child_ancestors.extend(self.access.ancestors_nearest_first(parent).await?);
        }
        let mut folders: Vec<DocumentFolder> = Vec::new();
        let mut after: Option<DocumentFolder> = None;
        for _ in 0..CHILD_FOLDER_PAGES {
            let page = self
                .repos
                .folders
                .list_visible_children(ChildFolderPage {
                    parent: request.folder.as_ref(),
                    caller: &who.id,
                    subjects: pairs,
                    pattern: pattern.as_deref(),
                    after: after.as_ref(),
                    limit: CHILD_FOLDER_PAGE,
                })
                .await?;
            let full = page.len() as i64 == CHILD_FOLDER_PAGE;
            after = page.last().cloned();
            for child in page {
                let admitted = if agent {
                    self.agent_child_admitted(who, &child, pairs, trace).await?
                } else {
                    let probe = FolderProbe {
                        folder: Some(&child.id),
                        pairs,
                        ancestors: &child_ancestors,
                    };
                    self.access.probe_folder_with(who, probe, trace).await
                };
                if admitted {
                    folders.push(child);
                }
            }
            if !full {
                return Ok(folders);
            }
        }
        tracing::warn!(
            folder_id = request.folder.as_ref().map(DocumentFolderId::as_str),
            shown = folders.len(),
            "Document listing truncated a folder's subfolders at the page bound"
        );
        Ok(folders)
    }

    async fn agent_child_admitted(
        &self,
        who: &AuthenticatedUser,
        child: &DocumentFolder,
        pairs: &SubjectPairs,
        trace: &TraceId,
    ) -> Result<bool, DocumentsError> {
        let target = AccessTarget::Folder(Some(child.id.clone()));
        let check = AccessCheck {
            who,
            target: &target,
            action: DocumentAction::View,
            trace,
        };
        match self.access.probe_with(check, pairs).await {
            Ok(_) => Ok(true),
            Err(DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }) => Ok(false),
            Err(other) => Err(other),
        }
    }

    pub(super) async fn hook_visible(
        &self,
        who: &AuthenticatedUser,
        documents: Vec<Document>,
        trace: &TraceId,
    ) -> Vec<Document> {
        self.hook_filter(who, documents, |d| d, trace).await
    }

    async fn hook_filter<T>(
        &self,
        who: &AuthenticatedUser,
        items: Vec<T>,
        document_of: impl Fn(&T) -> &Document,
        trace: &TraceId,
    ) -> Vec<T> {
        let mut allowed: HashMap<Option<DocumentFolderId>, bool> = HashMap::new();
        let mut admitted = Vec::with_capacity(items.len());
        for item in items {
            let key = document_of(&item).folder_id.clone();
            let admit = if let Some(admit) = allowed.get(&key) {
                *admit
            } else {
                let admit = self.access.probe_folder(who, key.as_ref(), trace).await;
                allowed.insert(key, admit);
                admit
            };
            if admit {
                admitted.push(item);
            }
        }
        let documents: Vec<&Document> = admitted.iter().map(&document_of).collect();
        let hidden = self
            .access
            .hidden_from_agent_many(who, &documents, trace)
            .await;
        admitted
            .into_iter()
            .zip(hidden)
            .filter_map(|(item, hidden)| (!hidden).then_some(item))
            .collect()
    }
}
