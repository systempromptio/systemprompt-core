//! Folder operations and folder listings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DocumentAction};

use super::support::validate_name;
use super::{DocumentService, NewFolderRequest};
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::models::{DocumentFolder, GrantTarget, NewFolder, NewGrant};

const DELETE_BATCH: i64 = 200;

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
        if let Err(error) = self.complete_folder(who, &folder).await {
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
    ) -> Result<(), DocumentsError> {
        self.access.register_folder_entity(&folder.id).await?;
        self.repos
            .grants
            .insert(NewGrant {
                target: GrantTarget::Folder(folder.id.clone()),
                subject_kind: "user".to_owned(),
                subject_value: who.id.as_str().to_owned(),
                subject_display: Some(who.username.clone()),
                access_level: AccessLevel::Share,
                granted_by: who.id.clone(),
                valid_until: None,
            })
            .await?;
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

    pub async fn move_folder(
        &self,
        who: &AuthenticatedUser,
        folder: &DocumentFolderId,
        parent: Option<&DocumentFolderId>,
        trace: &TraceId,
    ) -> Result<DocumentFolder, DocumentsError> {
        if parent == Some(folder) {
            return Err(DocumentsError::InvalidMove {
                folder_id: folder.clone(),
            });
        }
        self.access
            .authorize(
                who,
                AccessTarget::Folder(Some(folder.clone())),
                DocumentAction::Own,
                trace,
            )
            .await?;
        self.access
            .authorize(
                who,
                AccessTarget::Folder(parent.cloned()),
                DocumentAction::Create,
                trace,
            )
            .await?;
        if self.repos.folders.move_to(folder, parent).await? == 0 {
            return Err(DocumentsError::InvalidMove {
                folder_id: folder.clone(),
            });
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
        self.access
            .authorize(
                who,
                AccessTarget::Folder(Some(folder.clone())),
                DocumentAction::Delete,
                trace,
            )
            .await?;
        let subtree = self.subtree(folder).await?;
        if !recursive {
            let has_documents = !self
                .repos
                .documents
                .ids_in_folder(folder, 1)
                .await?
                .is_empty();
            if subtree.len() > 1 || has_documents {
                return Err(DocumentsError::FolderNotEmpty {
                    folder_id: folder.clone(),
                });
            }
        }
        for member in &subtree {
            self.delete_documents_in(who, member, trace).await?;
        }
        for member in subtree.iter().rev() {
            self.repos.folders.soft_delete(member).await?;
        }
        Ok(())
    }

    async fn delete_documents_in(
        &self,
        who: &AuthenticatedUser,
        folder: &DocumentFolderId,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        loop {
            let ids = self
                .repos
                .documents
                .ids_in_folder(folder, DELETE_BATCH)
                .await?;
            if ids.is_empty() {
                return Ok(());
            }
            for id in ids {
                self.access
                    .authorize(
                        who,
                        AccessTarget::Document(id.clone()),
                        DocumentAction::Delete,
                        trace,
                    )
                    .await?;
                self.repos.documents.trash(&id, &who.id).await?;
            }
        }
    }

    async fn subtree(
        &self,
        root: &DocumentFolderId,
    ) -> Result<Vec<DocumentFolderId>, DocumentsError> {
        let mut ordered = vec![root.clone()];
        let mut next = 0;
        while next < ordered.len() {
            let parent = ordered[next].clone();
            next += 1;
            for child in self.repos.folders.list_children(Some(&parent)).await? {
                ordered.push(child.id);
            }
        }
        Ok(ordered)
    }

    pub(super) async fn load_folder(
        &self,
        folder: &DocumentFolderId,
    ) -> Result<DocumentFolder, DocumentsError> {
        self.repos
            .folders
            .find(folder)
            .await?
            .filter(|f| f.deleted_at.is_none())
            .ok_or_else(|| DocumentsError::not_found("folder", folder))
    }
}
