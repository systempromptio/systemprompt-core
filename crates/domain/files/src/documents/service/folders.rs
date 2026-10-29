//! Folder operations and folder listings.
//!
//! A recursive delete authorizes the root and every folder below it through
//! the hook, partitions the documents into the caller's own (trashed) and
//! everyone else's (moved to their owners' roots), applies both in one
//! transaction that re-checks each owner, and then closes the live editing
//! state of every document it trashed. A delete whose teardown fails for a
//! document still trashed the subtree and reports that failure.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DocumentAction};
use systemprompt_traits::RepositoryError;

use super::lifecycle::TRASHED_NOTE;
use super::support::validate_name;
use super::{DocumentService, NewFolderRequest};
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentFolder, GrantTarget, NewFolder, NewGrant};
use crate::documents::repository::{SubtreeTrash, SubtreeTrashPlan};

const DELETE_BATCH: i64 = 500;

impl DocumentService {
    pub async fn create_folder(
        &self,
        who: &AuthenticatedUser,
        request: NewFolderRequest,
        trace: &TraceId,
    ) -> Result<DocumentFolder, DocumentsError> {
        validate_name(&request.name)?;
        self.access
            .authorize(
                who,
                AccessTarget::Folder(request.parent.clone()),
                DocumentAction::Create,
                trace,
            )
            .await?;
        let folder = self
            .repos
            .folders
            .insert(NewFolder {
                parent_id: request.parent,
                name: request.name,
                owner_user_id: who.id.clone(),
                created_by: who.id.clone(),
            })
            .await?;
        if let Err(error) = self.complete_folder(who, &folder, trace).await {
            if let Err(undo) = self.repos.folders.soft_delete(&folder.id).await {
                tracing::error!(
                    folder_id = %folder.id,
                    error = %undo,
                    "A folder whose registration failed could not be withdrawn"
                );
            }
            return Err(error);
        }
        Ok(folder)
    }

    async fn complete_folder(
        &self,
        who: &AuthenticatedUser,
        folder: &DocumentFolder,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        self.access.register_folder_entity(&folder.id).await?;
        let mut tx = self.repos.documents.begin().await?;
        self.repos
            .grants
            .insert(
                &mut tx,
                NewGrant {
                    target: GrantTarget::Folder(folder.id.clone()),
                    subject_kind: "user".to_owned(),
                    subject_value: who.id.as_str().to_owned(),
                    subject_display: Some(who.username.clone()),
                    access_level: AccessLevel::Share,
                    granted_by: who.id.clone(),
                    valid_until: None,
                    trace_id: trace.clone(),
                    reason: None,
                    via_agent: None,
                },
            )
            .await?;
        tx.commit().await.map_err(RepositoryError::from)?;
        Ok(())
    }

    pub async fn rename_folder(
        &self,
        who: &AuthenticatedUser,
        folder: &DocumentFolderId,
        name: &str,
        trace: &TraceId,
    ) -> Result<DocumentFolder, DocumentsError> {
        validate_name(name)?;
        self.access
            .authorize(
                who,
                AccessTarget::Folder(Some(folder.clone())),
                DocumentAction::Edit,
                trace,
            )
            .await?;
        if !self.repos.folders.rename(folder, name).await? {
            return Err(DocumentsError::not_found("folder", folder));
        }
        self.load_folder(folder).await
    }

    pub async fn delete_folder(
        &self,
        who: &AuthenticatedUser,
        folder: &DocumentFolderId,
        recursive: bool,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        let folders = self.repos.folders.subtree_folder_ids(folder).await?;
        let documents = self.subtree_documents(folder).await?;
        if !recursive && (folders.len() > 1 || !documents.is_empty()) {
            self.access
                .authorize(
                    who,
                    AccessTarget::Folder(Some(folder.clone())),
                    DocumentAction::Delete,
                    trace,
                )
                .await?;
            return Err(DocumentsError::FolderNotEmpty {
                folder_id: folder.clone(),
            });
        }
        self.access
            .authorize_subtree_folders(who, folder, &folders, trace)
            .await?;
        let partition = self
            .access
            .partition_subtree_documents(who, documents)
            .await?;
        let trash: Vec<DocumentId> = partition.trash.iter().map(|d| d.id.clone()).collect();
        let rehome: Vec<DocumentId> = partition.rehome.iter().map(|d| d.id.clone()).collect();
        match self
            .repos
            .folders
            .trash_subtree(
                folder,
                &who.id,
                SubtreeTrashPlan {
                    folders: &folders,
                    trash: &trash,
                    rehome: &rehome,
                },
            )
            .await?
        {
            SubtreeTrash::Trashed { .. } => {
                self.tear_down_trashed(who, &partition.trash, trace).await
            },
            SubtreeTrash::Changed => Err(DocumentsError::SubtreeChanged {
                folder_id: folder.clone(),
            }),
        }
    }

    async fn tear_down_trashed(
        &self,
        who: &AuthenticatedUser,
        documents: &[Document],
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        let mut first_failure = None;
        for document in documents {
            let torn = match self
                .reject_open_for_document(document, Some(TRASHED_NOTE.to_owned()))
                .await
            {
                Ok(_) => self.close_collab_state(who, document, trace).await,
                Err(error) => Err(error),
            };
            if let Err(error) = torn {
                tracing::error!(
                    document_id = %document.id,
                    error = %error,
                    "A document trashed with its folder kept live editing state"
                );
                first_failure.get_or_insert(error);
            }
        }
        let ids: Vec<DocumentId> = documents.iter().map(|d| d.id.clone()).collect();
        if let Err(error) = self.release_live_state(&ids).await {
            tracing::error!(
                folder_documents = ids.len(),
                error = %error,
                "Documents trashed with their folder kept locks or presence"
            );
            first_failure.get_or_insert(error);
        }
        first_failure.map_or(Ok(()), Err)
    }

    async fn subtree_documents(
        &self,
        root: &DocumentFolderId,
    ) -> Result<Vec<Document>, DocumentsError> {
        let mut documents: Vec<Document> = Vec::new();
        loop {
            let after = documents.last().map(|d| d.id.clone());
            let page = self
                .repos
                .folders
                .subtree_documents(root, after.as_ref(), DELETE_BATCH)
                .await?;
            let done = page.len() < usize::try_from(DELETE_BATCH).unwrap_or(usize::MAX);
            documents.extend(page);
            if done {
                return Ok(documents);
            }
        }
    }

    pub(super) async fn load_folder(
        &self,
        folder: &DocumentFolderId,
    ) -> Result<DocumentFolder, DocumentsError> {
        self.repos
            .folders
            .find(folder)
            .await?
            .filter(|f| f.trashed_at.is_none())
            .ok_or_else(|| DocumentsError::not_found("folder", folder))
    }
}
