//! Web presence in a live edit session: cursor updates and the presence
//! events announced to a document's other recipients.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::{DocumentEditSessionId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    DocumentAction, DocumentPresenceEvent, PresenceSource, PresenceState,
};

use super::{CollabError, CollabService, CursorRequest};
use crate::documents::models::{Document, PresenceUpsert};

pub(super) struct Announcement<'a> {
    pub(super) state: PresenceState,
    pub(super) session: &'a DocumentEditSessionId,
    // JSON: editor cursor position; the shape belongs to the client's editor.
    pub(super) cursor: Option<serde_json::Value>,
}

impl CollabService {
    pub async fn cursor(
        &self,
        who: &AuthenticatedUser,
        request: CursorRequest,
        trace: &TraceId,
    ) -> Result<(), CollabError> {
        let CursorRequest {
            document,
            session,
            cursor,
        } = request;
        let document = self
            .authorized(who, &document, DocumentAction::View, trace)
            .await?;
        self.open_matching(&document.id, &session).await?;
        let repos = self.documents.repositories();
        if !repos
            .collab
            .set_cursor(&document.id, &who.id, &cursor)
            .await?
        {
            return Err(CollabError::NotJoined {
                document_id: document.id,
            });
        }
        let state = if repos.collab.is_editing(&document.id, &who.id).await? {
            PresenceState::Editing
        } else {
            PresenceState::Viewing
        };
        self.announce(
            who,
            &document,
            Announcement {
                state,
                session: &session,
                cursor: Some(cursor),
            },
        )
        .await;
        Ok(())
    }

    pub(super) async fn mark_present(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        state: PresenceState,
        // JSON: editor cursor position; the shape belongs to the client's editor.
        cursor: Option<serde_json::Value>,
    ) -> Result<(), CollabError> {
        let ttl = Duration::from_secs(u64::from(self.documents.config().presence_ttl_secs));
        self.documents
            .repositories()
            .presence
            .upsert(
                &PresenceUpsert {
                    document_id: document.id.clone(),
                    user_id: who.id.clone(),
                    source: PresenceSource::Web,
                    state,
                    cursor,
                },
                ttl,
            )
            .await?;
        Ok(())
    }

    pub(super) async fn announce(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        Announcement {
            state,
            session,
            cursor,
        }: Announcement<'_>,
    ) {
        let others: Vec<UserId> = self
            .recipients(document)
            .await
            .into_iter()
            .filter(|user| *user != who.id)
            .collect();
        let event = DocumentPresenceEvent {
            document_id: document.id.clone(),
            user_id: who.id.clone(),
            state,
            source_kind: PresenceSource::Web,
            source_ref: session.to_string(),
            cursor,
        };
        if let Err(error) = self.documents.events().presence(&others, &event).await {
            tracing::warn!(document_id = %document.id, error = %error, "Presence event not delivered");
        }
    }

    pub(super) async fn recipients(&self, document: &Document) -> Vec<UserId> {
        self.documents.recipients(document).await
    }
}
