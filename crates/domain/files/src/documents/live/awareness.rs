//! Awareness: validating a participant's posted state, keeping it on their
//! web presence row until it expires, and publishing it on the live topic.
//!
//! A state that names the editing mode needs edit access, any other view
//! access. The presence row holds the last state even when publishing was
//! throttled, so a channel opened later starts from it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use chrono::Utc;
use systemprompt_identifiers::{DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::awareness::{AWARENESS_COLOR_MAX, AWARENESS_CURSOR_MAX};
use systemprompt_models::documents::{
    AwarenessCursor, AwarenessMode, AwarenessState, DocumentAction, DocumentAwarenessEvent,
    PresenceSource, PresenceState,
};

use super::DocumentService;
use crate::documents::error::{DocumentsError, EventFailure};
use crate::documents::models::PresenceUpsert;

pub const AWARENESS_SELECTION_MAX: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwarenessPublished {
    pub published: bool,
    pub event: DocumentAwarenessEvent,
}

impl DocumentService {
    pub async fn publish_awareness(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        state: AwarenessState,
        trace: &TraceId,
    ) -> Result<AwarenessPublished, DocumentsError> {
        validate(&state)?;
        let editing = state.mode == AwarenessMode::Editing;
        let action = if editing {
            DocumentAction::Edit
        } else {
            DocumentAction::View
        };
        let document = self.live_authorized(who, document, action, trace).await?;
        let ttl = Duration::from_secs(u64::from(self.config().presence_ttl_secs));
        let expires_at = Utc::now() + chrono::Duration::from_std(ttl).unwrap_or_default();
        let event = DocumentAwarenessEvent {
            document_id: document.id.clone(),
            user_id: who.id.clone(),
            display_name: who.username.clone(),
            state,
            expires_at,
        };
        let presence = PresenceUpsert {
            document_id: document.id,
            user_id: who.id.clone(),
            source: PresenceSource::Web,
            state: if editing {
                PresenceState::Editing
            } else {
                PresenceState::Viewing
            },
            cursor: Some(serde_json::to_value(&event).map_err(|source| {
                DocumentsError::Events {
                    event: DocumentAwarenessEvent::NAME,
                    source: EventFailure::Serialize(source),
                }
            })?),
        };
        self.repositories().presence.upsert(&presence, ttl).await?;
        let published = self.events().awareness(&event).await?;
        Ok(AwarenessPublished { published, event })
    }
}

fn validate(state: &AwarenessState) -> Result<(), DocumentsError> {
    let invalid = |detail| Err(DocumentsError::InvalidAwareness { detail });
    let color = &state.color;
    if color.is_empty() || color.len() > AWARENESS_COLOR_MAX {
        return invalid("color must be 1 to 32 characters");
    }
    if !color
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "#(),.% -".contains(c))
    {
        return invalid("color must be a CSS colour value");
    }
    if state.selection.len() > AWARENESS_SELECTION_MAX {
        return invalid("selection holds at most 16 ranges");
    }
    let oversized = |cursor: &AwarenessCursor| {
        cursor.anchor.len() > AWARENESS_CURSOR_MAX || cursor.head.len() > AWARENESS_CURSOR_MAX
    };
    if state.cursor.iter().chain(&state.selection).any(oversized) {
        return invalid("a cursor position is longer than 512 characters");
    }
    Ok(())
}
