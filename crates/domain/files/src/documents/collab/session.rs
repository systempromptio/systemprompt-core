//! Joining and leaving a live edit session, and cursor updates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use loro::ExportMode;
use systemprompt_identifiers::{DocumentEditSessionId, DocumentId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    DocumentAction, DocumentPresenceEvent, PresenceSource, PresenceState,
};

use super::model::is_collab_format;
use super::{CollabError, CollabService, CursorRequest, JoinMode, Joined, Left};

struct Announcement<'a> {
    state: PresenceState,
    session: &'a DocumentEditSessionId,
    // JSON: editor cursor position; the shape belongs to the client's editor.
    cursor: Option<serde_json::Value>,
}
use crate::documents::access::AccessTarget;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentEditSession, EditSessionOpen, PresenceUpsert};

impl CollabService {
    pub async fn join(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        mode: JoinMode,
        trace: &TraceId,
    ) -> Result<Joined, CollabError> {
        let action = match mode {
            JoinMode::Edit => DocumentAction::Edit,
            JoinMode::View => DocumentAction::View,
        };
        let document = self.authorized(who, document, action, trace).await?;
        if !is_collab_format(document.format) {
            return Err(DocumentsError::UnsupportedFormat {
                format: document.format,
            }
            .into());
        }
        let session = self.open_session(&document).await?;
        let entry = self.entry(&document, &session).await?;
        let mut entry = entry.lock().await;
        self.catch_up(&mut entry).await?;
        let joined = Joined {
            session_id: session.id.clone(),
            snapshot: entry.doc.export(ExportMode::Snapshot)?,
            version: entry.doc.oplog_vv().encode(),
            head_revision: document.head_revision_id.clone(),
        };
        drop(entry);
        let state = match mode {
            JoinMode::Edit => PresenceState::Editing,
            JoinMode::View => PresenceState::Viewing,
        };
        self.mark_present(who, &document, state, None).await?;
        self.announce(
            who,
            &document,
            Announcement {
                state,
                session: &session.id,
                cursor: None,
            },
        )
        .await;
        Ok(joined)
    }

    pub async fn leave(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        session: &DocumentEditSessionId,
        trace: &TraceId,
    ) -> Result<Left, CollabError> {
        let document = self
            .authorized(who, document, DocumentAction::View, trace)
            .await?;
        let repos = self.documents.repositories();
        repos
            .collab
            .remove_web_presence(&document.id, &who.id)
            .await?;
        let open = repos.edit_sessions.find_open(&document.id).await?;
        let Some(open) = open.filter(|open| open.id == *session) else {
            return Ok(Left {
                closed: false,
                checkpoint: None,
            });
        };
        if repos.collab.count_editors(&document.id).await? > 0 {
            return Ok(Left {
                closed: false,
                checkpoint: None,
            });
        }
        let entry = self.entry(&document, &open).await?;
        let mut entry = entry.lock().await;
        let checkpoint = self.checkpoint(&mut entry, who, trace).await?;
        let closed = repos.edit_sessions.close(&open.id).await?;
        drop(entry);
        self.cache_remove(&document.id);
        Ok(Left { closed, checkpoint })
    }

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

    pub(super) async fn authorized(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        action: DocumentAction,
        trace: &TraceId,
    ) -> Result<Document, CollabError> {
        self.documents
            .access()
            .authorize(who, AccessTarget::Document(document.clone()), action, trace)
            .await?
            .document
            .ok_or_else(|| DocumentsError::not_found("document", document).into())
    }

    pub(super) async fn open_matching(
        &self,
        document: &DocumentId,
        session: &DocumentEditSessionId,
    ) -> Result<DocumentEditSession, CollabError> {
        self.documents
            .repositories()
            .edit_sessions
            .find_open(document)
            .await?
            .filter(|open| open.id == *session)
            .ok_or_else(|| CollabError::SessionClosed {
                document_id: document.clone(),
                session_id: session.clone(),
            })
    }

    async fn open_session(&self, document: &Document) -> Result<DocumentEditSession, CollabError> {
        let sessions = &self.documents.repositories().edit_sessions;
        if let Some(open) = sessions.find_open(&document.id).await? {
            return Ok(open);
        }
        match sessions
            .open(&document.id, document.head_revision_id.as_ref())
            .await?
        {
            EditSessionOpen::Opened(session) => Ok(session),
            EditSessionOpen::AlreadyOpen => {
                sessions.find_open(&document.id).await?.ok_or_else(|| {
                    DocumentsError::not_found("edit session of document", &document.id).into()
                })
            },
        }
    }

    async fn mark_present(
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

    async fn announce(
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
