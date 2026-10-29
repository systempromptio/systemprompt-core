//! Closing a document's live edit session when the document is trashed.
//!
//! The session's live body is settled as the most recent author the update
//! log records (see the `committer` module), so a body that conflicts with
//! the head is kept as a proposal. A session that
//! settling does not close, because another replica holds its checkpoint
//! lease or updates kept arriving, is then closed whatever the outcome.
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
            let mut closed = false;
            if is_collab_format(document.format) {
                let entry = self.entry(document, &open).await?;
                match self.settle(&entry, who, trace).await {
                    Ok(left) => {
                        closed = left.closed;
                        checkpoint = left.checkpoint;
                    },
                    Err(CollabError::CheckpointBusy { .. }) => {
                        tracing::warn!(
                            document_id = %document.id,
                            session_id = %open.id,
                            "Edit session of a trashed document closed while another replica held its checkpoint lease"
                        );
                    },
                    Err(error) => return Err(error),
                }
            }
            if !closed {
                repos.edit_sessions.close(&open.id).await?;
            }
            self.cache_remove(&document.id);
        }
        Ok(checkpoint)
    }
}
