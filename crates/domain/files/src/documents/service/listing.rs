//! Folder listings and the hook filter applied to every document listing.
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
use crate::documents::models::{Document, VisibleListing};

impl DocumentService {
    pub async fn list_folder(
        &self,
        who: &AuthenticatedUser,
        request: ListFolderRequest,
        trace: &TraceId,
    ) -> Result<Listing, DocumentsError> {
        let ListFolderRequest {
            folder,
            cursor,
            limit,
        } = request;
        let folder = folder.as_ref();
        let granted = self
            .access
            .authorize(
                who,
                AccessTarget::Folder(folder.cloned()),
                DocumentAction::View,
                trace,
            )
            .await?;
        let mut folders = Vec::new();
        if cursor.is_none() {
            for child in self.repos.folders.list_children(folder).await? {
                let target = AccessTarget::Folder(Some(child.id.clone()));
                let check = AccessCheck {
                    who,
                    target: &target,
                    action: DocumentAction::View,
                    trace,
                };
                let probe = self.access.probe_with(check, &granted.pairs).await;
                match probe {
                    Ok(_) => folders.push(child),
                    Err(DocumentsError::Forbidden { .. }) => {},
                    Err(other) => return Err(other),
                }
            }
        }
        let (page, next_cursor) = self
            .repos
            .documents
            .list_visible(
                VisibleListing {
                    folder_id: folder.cloned(),
                    query: None,
                    cursor,
                    limit,
                },
                &granted.pairs,
                &who.id,
            )
            .await?;
        let page = page
            .into_iter()
            .filter(|d| folder.is_some() || d.folder_id.is_none())
            .collect();
        let documents = self.hook_visible(who, page, trace).await;
        Ok(Listing {
            folders,
            documents,
            next_cursor,
        })
    }

    pub(super) async fn hook_visible(
        &self,
        who: &AuthenticatedUser,
        documents: Vec<Document>,
        trace: &TraceId,
    ) -> Vec<Document> {
        let mut allowed: HashMap<Option<DocumentFolderId>, bool> = HashMap::new();
        let mut visible = Vec::with_capacity(documents.len());
        for document in documents {
            let key = document.folder_id.clone();
            let admit = if let Some(admit) = allowed.get(&key) {
                *admit
            } else {
                let admit = self.access.probe_folder(who, key.as_ref(), trace).await;
                allowed.insert(key, admit);
                admit
            };
            if admit {
                visible.push(document);
            }
        }
        visible
    }
}
