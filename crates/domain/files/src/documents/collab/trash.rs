//! Closing a document's live edit session when the document is trashed.
//!
//! The session's live body is settled as its last editor (or, when this
//! replica has not seen an editor, as the user trashing the document), so a
//! body that conflicts with the head is kept as a proposal; the session is
//! then closed whatever the outcome.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;

use super::model::is_collab_format;
use super::{Checkpoint, CollabError, CollabService};
use crate::documents::models::Document;

impl CollabService {
    pub async fn close_for_trash(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        trace: &TraceId,
    ) -> Result<Checkpoint, CollabError> {
        let repos = self.documents.repositories();
        let mut checkpoint = Checkpoint::Unchanged;
        if let Some(open) = repos.edit_sessions.find_open(&document.id).await? {
            if is_collab_format(document.format) {
                let entry = self.entry(document, &open).await?;
                let mut entry = entry.lock().await;
                let editor = entry.last_editor.clone().unwrap_or_else(|| who.clone());
                checkpoint = self.settle(&mut entry, &editor, trace).await?;
                drop(entry);
            }
            repos.edit_sessions.close(&open.id).await?;
            self.cache_remove(&document.id);
        }
        Ok(checkpoint)
    }
}
