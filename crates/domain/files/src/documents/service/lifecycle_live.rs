//! Tearing down a trashed document's live state: its collaborative edit
//! session, locks and presence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;

use super::DocumentService;
use crate::documents::error::DocumentsError;
use crate::documents::models::Document;

impl DocumentService {
    pub(super) async fn close_live_state(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        self.close_collab_state(who, document, trace).await?;
        self.release_live_state(std::slice::from_ref(&document.id))
            .await
    }

    #[cfg(feature = "collab")]
    pub(super) async fn close_collab_state(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        if let Some(collab) = self.collab() {
            collab
                .close_for_trash(who, document, trace)
                .await
                .map_err(|source| DocumentsError::LiveSession(Box::new(source)))?;
        }
        Ok(())
    }

    #[cfg(not(feature = "collab"))]
    #[expect(
        clippy::unused_async,
        reason = "shares the signature of the collab build"
    )]
    pub(super) async fn close_collab_state(
        &self,
        _who: &AuthenticatedUser,
        _document: &Document,
        _trace: &TraceId,
    ) -> Result<(), DocumentsError> {
        Ok(())
    }

    pub(super) async fn release_live_state(
        &self,
        documents: &[DocumentId],
    ) -> Result<(), DocumentsError> {
        self.repos.locks.release_all_for(documents).await?;
        self.repos.presence.clear_for(documents).await?;
        Ok(())
    }
}
