//! The live-session events emitted through [`DocumentEvents`] beside the
//! engine updates: a checkpoint that saved placeholders for images its
//! inserters may not place, sent to the document's recipients.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;
use systemprompt_models::documents::DocumentCollabDegradedEvent;

use super::error::DocumentsError;
use super::events::DocumentEvents;

impl DocumentEvents {
    pub async fn collab_degraded(
        &self,
        to: &[UserId],
        event: &DocumentCollabDegradedEvent,
    ) -> Result<(), DocumentsError> {
        self.emit(DocumentCollabDegradedEvent::NAME, to, event)
            .await
    }
}
