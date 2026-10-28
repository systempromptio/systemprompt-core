//! Reading access back: who holds what on a document or a folder.
//!
//! A caller below `share` reads only the owner and their own level, never
//! the other subjects, the audit log or the open access requests. A grant
//! inherited from an ancestor folder names that folder. The audit log is
//! read only on request, newest first and capped at
//! [`MAX_GRANT_HISTORY`] rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::{DocumentFolderId, DocumentId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DocumentAction};

use super::DocumentService;
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::models::{
    Document, DocumentAccessRequest, DocumentFolder, DocumentGrant, DocumentGrantEvent, GrantTarget,
};

pub const MAX_GRANT_HISTORY: i64 = 100;

/// A grant on an ancestor folder, with that folder's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InheritedGrant {
    pub grant: DocumentGrant,
    pub folder_id: DocumentFolderId,
    pub folder_name: String,
}

/// Who holds what on one document or folder.
///
/// Its owner, its own grants, the grants it inherits from its folder chain,
/// the caller's level and, for a caller holding `share`, the audit log when
/// asked for and (documents only) the open access requests. Everything but
/// the owner and the caller's level is empty below `share`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetAccess<T> {
    pub target: T,
    pub owner: UserId,
    pub grants: Vec<DocumentGrant>,
    pub inherited: Vec<InheritedGrant>,
    pub my_access: AccessLevel,
    pub history: Vec<DocumentGrantEvent>,
    pub requests: Vec<DocumentAccessRequest>,
}

pub type DocumentAccess = TargetAccess<Document>;
pub type FolderAccess = TargetAccess<DocumentFolder>;

impl DocumentService {
    pub async fn access_of(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        history: bool,
        trace: &TraceId,
    ) -> Result<DocumentAccess, DocumentsError> {
        let granted = self
            .access
            .authorize(
                who,
                AccessTarget::Document(document.clone()),
                DocumentAction::View,
                trace,
            )
            .await?;
        let current = granted
            .document
            .ok_or_else(|| DocumentsError::not_found("document", document))?;
        let mut access = TargetAccess::bare(current.owner_user_id.clone(), granted.level, current);
        if access.my_access < AccessLevel::Share {
            return Ok(access);
        }
        let target = GrantTarget::Document(document.clone());
        access.grants = self.list_grants(&target).await?;
        if let Some(folder) = &access.target.folder_id {
            let mut chain = vec![folder.clone()];
            chain.extend(self.repos.folders.ancestors(folder).await?);
            access.inherited = self.inherited_from(&chain).await?;
        }
        access.history = self.history_if(history, &target).await?;
        access.requests = self.repos.access_requests.list_open(document).await?;
        Ok(access)
    }

    pub async fn folder_access_of(
        &self,
        who: &AuthenticatedUser,
        folder: &DocumentFolderId,
        history: bool,
        trace: &TraceId,
    ) -> Result<FolderAccess, DocumentsError> {
        let granted = self
            .access
            .authorize(
                who,
                AccessTarget::Folder(Some(folder.clone())),
                DocumentAction::View,
                trace,
            )
            .await?;
        let current = granted
            .folder
            .ok_or_else(|| DocumentsError::not_found("folder", folder))?;
        let mut access = TargetAccess::bare(current.owner_user_id.clone(), granted.level, current);
        if access.my_access < AccessLevel::Share {
            return Ok(access);
        }
        let target = GrantTarget::Folder(folder.clone());
        access.grants = self.list_grants(&target).await?;
        let chain = self.repos.folders.ancestors(folder).await?;
        access.inherited = self.inherited_from(&chain).await?;
        access.history = self.history_if(history, &target).await?;
        Ok(access)
    }

    async fn inherited_from(
        &self,
        chain: &[DocumentFolderId],
    ) -> Result<Vec<InheritedGrant>, DocumentsError> {
        let mut names = HashMap::new();
        for folder in chain {
            if let Some(found) = self.repos.folders.find(folder).await? {
                names.insert(found.id, found.name);
            }
        }
        Ok(self
            .repos
            .grants
            .list_for_folders(chain)
            .await?
            .into_iter()
            .filter_map(|grant| {
                let GrantTarget::Folder(folder_id) = grant.target.clone() else {
                    return None;
                };
                let folder_name = names.get(&folder_id)?.clone();
                Some(InheritedGrant {
                    grant,
                    folder_id,
                    folder_name,
                })
            })
            .collect())
    }

    async fn history_if(
        &self,
        history: bool,
        target: &GrantTarget,
    ) -> Result<Vec<DocumentGrantEvent>, DocumentsError> {
        if !history {
            return Ok(Vec::new());
        }
        Ok(self
            .repos
            .grant_events
            .list_for_target(target, MAX_GRANT_HISTORY)
            .await?)
    }
}

impl<T> TargetAccess<T> {
    const fn bare(owner: UserId, my_access: AccessLevel, target: T) -> Self {
        Self {
            target,
            owner,
            grants: Vec::new(),
            inherited: Vec::new(),
            my_access,
            history: Vec::new(),
            requests: Vec::new(),
        }
    }
}
