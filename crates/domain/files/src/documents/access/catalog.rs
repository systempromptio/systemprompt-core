//! Folders as access-control entities.
//!
//! Every folder has an `access_control_entities` row (`entity_type =
//! 'folder'`, `default_included = true`, `source = 'code:files'`), and so
//! does the root ([`root_folder_entity`]), so operator rules can target any
//! folder. A row that already exists is left untouched: an operator or a
//! bundle may have set its `default_included` or `source`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_governance::authz::EntityKind;
use systemprompt_identifiers::DocumentFolderId;

use super::service::{DocumentAccessService, root_folder_entity};
use crate::documents::error::DocumentsError;

const FOLDER_ENTITY_SOURCE: &str = "code:files";

impl DocumentAccessService {
    pub async fn register_folder_entity(
        &self,
        folder: &DocumentFolderId,
    ) -> Result<(), DocumentsError> {
        let existing = self
            .access_entities
            .get_entity(EntityKind::Folder, folder.as_str())
            .await?;
        if existing.is_none() {
            self.access_entities
                .upsert_entity(
                    EntityKind::Folder,
                    folder.as_str(),
                    true,
                    FOLDER_ENTITY_SOURCE,
                )
                .await?;
        }
        Ok(())
    }

    pub async fn reconcile_folder_entities(&self) -> Result<u64, DocumentsError> {
        let registered: HashSet<String> = self
            .access_entities
            .list_entities(EntityKind::Folder)
            .await?
            .into_iter()
            .map(|row| row.id)
            .collect();
        let mut pending = vec![root_folder_entity()];
        let mut frontier: Vec<Option<DocumentFolderId>> = vec![None];
        while let Some(parent) = frontier.pop() {
            for child in self.folders.list_children(parent.as_ref()).await? {
                frontier.push(Some(child.id.clone()));
                pending.push(child.id);
            }
        }
        let mut inserted = 0;
        for folder in pending {
            if registered.contains(folder.as_str()) {
                continue;
            }
            self.access_entities
                .upsert_entity(
                    EntityKind::Folder,
                    folder.as_str(),
                    true,
                    FOLDER_ENTITY_SOURCE,
                )
                .await?;
            inserted += 1;
        }
        Ok(inserted)
    }
}
