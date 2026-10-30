//! Joining and leaving a live edit session, and cursor updates.
//!
//! The last editor leaving settles the session and closes it, unless another
//! replica holds its checkpoint lease or updates keep arriving while it is
//! settled; such a session stays open until it is left again or the idle
//! session reaper settles it. Leaving a session that is already closed, or
//! that another replica closed while this leave settled it, answers that it
//! closed. A join that finds its session closed while binding to it (its
//! state was unreadable and the session was closed) joins the new session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_identifiers::{DocumentEditSessionId, DocumentId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    AccessLevel, DocumentAction, DocumentPresenceEvent, PresenceSource, PresenceState,
};

use super::model::is_collab_format;
use super::{Checkpoint, CollabError, CollabService, CursorRequest, JoinMode, Joined, Left};
use crate::documents::engine::DocumentEngine;
use crate::documents::error::DocumentsError;
use crate::documents::model::RICH_TEXT_VERSION;
use crate::documents::models::{Document, DocumentEditSession, EditSessionOpen, PresenceUpsert};

struct Announcement<'a> {
    state: PresenceState,
    session: &'a DocumentEditSessionId,
    // JSON: editor cursor position; the shape belongs to the client's editor.
    cursor: Option<serde_json::Value>,
}

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
            JoinMode::Comment => DocumentAction::Comment,
            JoinMode::View => DocumentAction::View,
        };
        let engine = self.engine()?;
        let (document, level) = self.granted(who, document, action, trace).await?;
        if !is_collab_format(document.format) {
            return Err(DocumentsError::UnsupportedFormat {
                format: document.format,
            }
            .into());
        }
        let joined = match self.bind(who, &document, level, engine.as_ref()).await {
            Err(CollabError::SessionClosed { .. }) => {
                self.bind(who, &document, level, engine.as_ref()).await
            },
            other => other,
        }?;
        let session_id = joined.session_id.clone();
        let state = match mode {
            JoinMode::Edit | JoinMode::Comment => PresenceState::Editing,
            JoinMode::View => PresenceState::Viewing,
        };
        self.mark_present(who, &document, state, None).await?;
        self.announce(
            who,
            &document,
            Announcement {
                state,
                session: &session_id,
                cursor: None,
            },
        )
        .await;
        Ok(joined)
    }

    async fn bind(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        level: AccessLevel,
        engine: &dyn DocumentEngine,
    ) -> Result<Joined, CollabError> {
        let session = self.open_session(document).await?;
        let peer = self.assign_peer(&session.id, &who.id).await?;
        let shared = self.entry(document, &session).await?;
        shared.lock().await.last_joiner = Some(who.clone());
        if let Err(error) = self.upgrade_shape(&shared, document, who).await {
            tracing::warn!(
                session_id = %session.id,
                error = %error,
                "Live session not upgraded to the current container shape"
            );
        }
        let entry = self.caught_up(&shared).await?;
        Ok(Joined {
            peer,
            session_id: session.id.clone(),
            snapshot: entry.doc.snapshot()?,
            version: entry.doc.version(),
            seq: entry.last_seq,
            head_revision: document.head_revision_id.clone(),
            engine: engine.name(),
            wire_version: engine.wire_version(),
            model: entry.doc.export_model()?.kind(),
            model_version: RICH_TEXT_VERSION,
            level,
        })
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
        let was_editing = repos.collab.is_editing(&document.id, &who.id).await?;
        repos
            .collab
            .remove_web_presence(&document.id, &who.id)
            .await?;
        let stays_open = Left {
            closed: false,
            checkpoint: Checkpoint::Unchanged,
        };
        let open = repos.edit_sessions.find_open(&document.id).await?;
        let Some(open) = open.filter(|open| open.id == *session) else {
            return Ok(already_closed());
        };
        if repos.collab.count_editors(&document.id).await? > 0 {
            return Ok(stays_open);
        }
        let entry = match self.entry(&document, &open).await {
            Err(CollabError::SessionClosed { .. }) => return Ok(already_closed()),
            other => other?,
        };
        if !was_editing && entry.lock().await.last_editor.is_none() {
            return Ok(stays_open);
        }
        let settled = match self.settle(&entry, who, trace).await {
            Err(CollabError::CheckpointLeaseLost { .. }) => self.settle(&entry, who, trace).await,
            other => other,
        };
        let left = match settled {
            Err(CollabError::CheckpointBusy { .. }) => stays_open,
            Err(CollabError::SessionClosed { .. }) => already_closed(),
            Err(CollabError::CheckpointLeaseLost { .. }) => {
                if repos
                    .edit_sessions
                    .find_open(&document.id)
                    .await?
                    .is_some_and(|still| still.id == open.id)
                {
                    stays_open
                } else {
                    already_closed()
                }
            },
            other => other?,
        };
        if left.closed {
            self.cache_remove(&document.id);
        }
        Ok(left)
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

const fn already_closed() -> Left {
    Left {
        closed: true,
        checkpoint: Checkpoint::Unchanged,
    }
}
