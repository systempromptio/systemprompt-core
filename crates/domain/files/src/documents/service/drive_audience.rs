//! Who a `document.drive.changed` hint reaches, and the hints that follow a
//! change of metadata or access.
//!
//! A document's audience is its access holders now. A folder's audience is
//! its owner and the members of every grant subject on the folder, its
//! ancestors and the folders below it. A change that takes access away
//! (a move, a trash, a removed grant, a deleted or moved folder) also
//! reaches the users who held it before: the document's access holders read
//! before the write, or the members of the removed grants' subjects. A
//! deleted folder's former audience includes the owners of the documents in
//! it. The whole audience is capped at `MAX_SUBJECT_MEMBERS`, the
//! document's owner and the actor first, then the rest in user-id order.
//!
//! Reading the former holders never fails the write it precedes: a failure
//! is logged and the hint is withheld.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_identifiers::{DocumentFolderId, DocumentId, UserId};

use super::DocumentService;
use super::drive_hint::{DriveHint, FormerHolders, HintScope};
use super::ownership::USER_SUBJECT;
use crate::documents::access::MAX_SUBJECT_MEMBERS;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentGrant};

fn capped(hint: &DriveHint, mut users: BTreeSet<UserId>) -> Vec<UserId> {
    let mut kept = Vec::new();
    if let HintScope::Document(document) = &hint.scope
        && users.remove(&document.owner_user_id)
    {
        kept.push(document.owner_user_id.clone());
    }
    if users.remove(&hint.actor) {
        kept.push(hint.actor.clone());
    }
    let room = MAX_SUBJECT_MEMBERS.saturating_sub(kept.len());
    if users.len() > room {
        tracing::warn!(
            target_id = ?hint.key,
            limit = MAX_SUBJECT_MEMBERS,
            candidates = users.len() + kept.len(),
            "Drive-change hint has more recipients than the fan-out limit; the rest rely on their poll"
        );
    }
    kept.extend(users.into_iter().take(room));
    kept
}

impl DocumentService {
    pub(super) async fn hint_audience(
        &self,
        hint: &DriveHint,
    ) -> Result<Vec<UserId>, DocumentsError> {
        let mut users = hint.also.clone();
        match &hint.scope {
            HintScope::Document(document) => {
                users.extend(self.access_holders(document, &hint.actor).await?);
            },
            HintScope::Folder(folder) => users.extend(self.folder_holders(folder).await?),
            HintScope::Former => {},
        }
        Ok(capped(hint, users))
    }

    async fn folder_holders(
        &self,
        folder: &DocumentFolderId,
    ) -> Result<BTreeSet<UserId>, DocumentsError> {
        let mut chain = self.repos.folders.subtree_folder_ids(folder).await?;
        chain.extend(self.repos.folders.ancestors(folder).await?);
        if !chain.contains(folder) {
            chain.push(folder.clone());
        }
        let grants = self.repos.grants.list_for_folders(&chain).await?;
        let mut users = self.grant_users(&grants).await?;
        if let Some(found) = self.repos.folders.find(folder).await? {
            users.insert(found.owner_user_id);
        }
        Ok(users)
    }

    async fn grant_users(
        &self,
        grants: &[DocumentGrant],
    ) -> Result<BTreeSet<UserId>, DocumentsError> {
        let mut users = BTreeSet::new();
        for grant in grants {
            if grant.subject_kind == USER_SUBJECT {
                users.insert(UserId::new(grant.subject_value.clone()));
                continue;
            }
            users.extend(
                self.access
                    .subject_members(&grant.subject_kind, &grant.subject_value)
                    .await?,
            );
        }
        Ok(users)
    }

    pub(super) async fn holders_before(
        &self,
        document: &Document,
        actor: &UserId,
    ) -> FormerHolders {
        match self.access_holders(document, actor).await {
            Ok(holders) => FormerHolders(Some(holders.into_iter().collect())),
            Err(error) => {
                tracing::warn!(
                    document_id = %document.id,
                    error = %error,
                    "Drive-change hint withheld: the access holders before the change could not be resolved"
                );
                FormerHolders(None)
            },
        }
    }

    pub(super) async fn folder_holders_before(
        &self,
        folder: &DocumentFolderId,
        documents: &[Document],
    ) -> FormerHolders {
        match self.folder_holders(folder).await {
            Ok(mut holders) => {
                holders.extend(documents.iter().map(|d| d.owner_user_id.clone()));
                FormerHolders(Some(holders))
            },
            Err(error) => {
                tracing::warn!(
                    folder_id = %folder,
                    error = %error,
                    "Drive-change hint withheld: the folder's audience before the change could not be resolved"
                );
                FormerHolders(None)
            },
        }
    }

    pub(super) async fn hinted(
        &self,
        document: &DocumentId,
        actor: &UserId,
    ) -> Result<Document, DocumentsError> {
        let document = self.load_document(document).await?;
        self.hint_drive_changed(DriveHint::document(&document, actor));
        Ok(document)
    }

    pub(super) async fn hinted_after(
        &self,
        document: &DocumentId,
        actor: &UserId,
        before: FormerHolders,
    ) -> Result<Document, DocumentsError> {
        let document = self.load_document(document).await?;
        self.hint_drive_changed(DriveHint::document(&document, actor).with(before));
        Ok(document)
    }

    pub(super) async fn hint_after_revoke(
        &self,
        hint: Option<DriveHint>,
        grants: &[DocumentGrant],
    ) {
        let Some(hint) = hint else {
            return;
        };
        let before = match self.grant_users(grants).await {
            Ok(users) => FormerHolders(Some(users)),
            Err(error) => {
                tracing::warn!(
                    target_id = ?hint.key,
                    error = %error,
                    "Drive-change hint withheld: the revoked subjects' users could not be resolved"
                );
                FormerHolders(None)
            },
        };
        self.hint_drive_changed(hint.with(before));
    }
}
