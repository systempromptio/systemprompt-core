//! The sharing events, emitted through [`DocumentEvents`]: a share reaches
//! its recipients only, since it names them and may carry a private
//! message; an access request and an ownership transfer are also published
//! once on the document's live topic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;
use systemprompt_models::documents::{
    DocumentAccessRequestedEvent, DocumentOwnershipTransferredEvent, DocumentSharedEvent,
};

use super::error::DocumentsError;
use super::events::{Audience, DocumentEvents};

impl DocumentEvents {
    pub async fn shared(
        &self,
        to: &[UserId],
        event: &DocumentSharedEvent,
    ) -> Result<(), DocumentsError> {
        self.emit_to(DocumentSharedEvent::NAME, to, event, Audience::Recipients)
            .await
    }

    pub async fn access_requested(
        &self,
        to: &[UserId],
        event: &DocumentAccessRequestedEvent,
    ) -> Result<(), DocumentsError> {
        self.emit(DocumentAccessRequestedEvent::NAME, to, event)
            .await
    }

    pub async fn ownership_transferred(
        &self,
        to: &[UserId],
        event: &DocumentOwnershipTransferredEvent,
    ) -> Result<(), DocumentsError> {
        self.emit(DocumentOwnershipTransferredEvent::NAME, to, event)
            .await
    }
}
