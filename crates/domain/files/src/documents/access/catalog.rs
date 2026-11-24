//! Folders as access-control entities.
//!
//! Every folder has an `access_control_entities` row (`entity_type =
//! 'folder'`, `default_included = true`, `source = 'code:files'`), and so
//! does the root ([`root_folder_entity`]), so operator rules can target any
//! folder. A row that already exists is left untouched: an operator or a
//! bundle may have set its `default_included` or `source`. The reconcile
//! reads every live folder reachable from the root in one recursive query and
//! inserts the missing rows in batches of at most 5,000, so a tree of
//! 4,000 folders costs two statements rather than one per folder.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
        self.access_entities
            .insert_missing_entities(
                EntityKind::Folder,
                &[folder.as_str()],
                true,
                FOLDER_ENTITY_SOURCE,
            )
            .await?;
        Ok(())
    }

    pub async fn reconcile_folder_entities(&self) -> Result<u64, DocumentsError> {
        let mut folders = vec![root_folder_entity()];
        folders.extend(self.folders.list_live_folder_ids().await?);
        let ids: Vec<&str> = folders.iter().map(DocumentFolderId::as_str).collect();
        let inserted = self
            .access_entities
            .insert_missing_entities(EntityKind::Folder, &ids, true, FOLDER_ENTITY_SOURCE)
            .await?;
        Ok(inserted)
    }
}
