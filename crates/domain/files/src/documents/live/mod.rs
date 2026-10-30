//! The document-scoped live channel, engine-agnostic.
//!
//! A live channel subscribes to the document's topic
//! ([`live_topic`](super::events::live_topic)), which carries every document
//! event once whoever its recipients were, relayed across replicas by the
//! event router. A channel subscribes before [`DocumentService::live_open`],
//! which gates it on view access and returns what it starts with: the open
//! edit session, the last sequence number that session had logged, and the
//! awareness states still alive. [`DocumentService::live_replay`] turns a
//! client's `Last-Event-ID` (`<session>:<seq>`, the highest sequence number
//! below which the channel delivered every update) into the collab updates
//! after it, or a [`LiveResyncReason`] when the log can no longer supply
//! them.
//! [`LiveCursor`] tracks what a channel delivered and names its resume id.
//! The `awareness` submodule validates and publishes awareness states.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod awareness;
mod cursor;

use systemprompt_identifiers::{DocumentEditSessionId, DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    DocumentAction, DocumentAwarenessEvent, DocumentCollabUpdateEvent, LiveResyncReason,
};

pub use awareness::{AWARENESS_SELECTION_MAX, AwarenessPublished};
pub use cursor::{LiveCursor, LiveDelivery};

use super::access::AccessTarget;
use super::error::DocumentsError;
use super::models::Document;
use super::service::DocumentService;

/// Where a resumed channel left off: every collab update of `session` up to
/// and including `seq` was delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeToken {
    pub session: DocumentEditSessionId,
    pub seq: i64,
}

impl ResumeToken {
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let (session, seq) = raw.rsplit_once(':')?;
        Some(Self {
            session: DocumentEditSessionId::try_new(session).ok()?,
            seq: seq.parse().ok()?,
        })
    }
}

impl std::fmt::Display for ResumeToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.session, self.seq)
    }
}

#[derive(Debug, Clone)]
pub struct LiveOpen {
    pub document: Document,
    pub edit_session: Option<DocumentEditSessionId>,
    pub seq: Option<i64>,
    pub awareness: Vec<DocumentAwarenessEvent>,
}

#[derive(Debug, Clone)]
pub enum LiveReplay {
    Updates(Vec<DocumentCollabUpdateEvent>),
    Resync(LiveResyncReason),
}

impl DocumentService {
    pub async fn live_open(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<LiveOpen, DocumentsError> {
        let document = self
            .live_authorized(who, document, DocumentAction::View, trace)
            .await?;
        let repos = self.repositories();
        let edit_session = repos
            .edit_sessions
            .find_open(&document.id)
            .await?
            .map(|open| open.id);
        let seq = match &edit_session {
            Some(session) => repos.collab.head_seq(session).await?,
            None => None,
        };
        let awareness = repos
            .presence
            .list_active(&document.id)
            .await?
            .into_iter()
            .filter_map(|row| row.cursor)
            .filter_map(|cursor| serde_json::from_value::<DocumentAwarenessEvent>(cursor).ok())
            .collect();
        Ok(LiveOpen {
            document,
            edit_session,
            seq,
            awareness,
        })
    }

    pub async fn live_allowed(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<bool, DocumentsError> {
        match self
            .live_authorized(who, document, DocumentAction::View, trace)
            .await
        {
            Ok(_) => Ok(true),
            Err(DocumentsError::Forbidden { .. } | DocumentsError::NotFound { .. }) => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub async fn live_replay(
        &self,
        open: &LiveOpen,
        resume: &ResumeToken,
    ) -> Result<LiveReplay, DocumentsError> {
        if open.edit_session.as_ref() != Some(&resume.session) {
            return Ok(LiveReplay::Resync(LiveResyncReason::SessionChanged));
        }
        let collab = &self.repositories().collab;
        let compacted = collab
            .snapshot_seq(&resume.session)
            .await?
            .is_some_and(|covered| covered > resume.seq);
        if compacted {
            return Ok(LiveReplay::Resync(LiveResyncReason::UpdatesCompacted));
        }
        let rows = collab.list_after(&resume.session, resume.seq).await?;
        Ok(LiveReplay::Updates(
            rows.into_iter()
                .map(|row| DocumentCollabUpdateEvent {
                    document_id: open.document.id.clone(),
                    edit_session_id: resume.session.clone(),
                    seq: row.seq,
                    update_b64: base64_encode(&row.update),
                    author_id: row.author_user_id,
                })
                .collect(),
        ))
    }

    async fn live_authorized(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        action: DocumentAction,
        trace: &TraceId,
    ) -> Result<Document, DocumentsError> {
        self.access()
            .authorize(who, AccessTarget::Document(document.clone()), action, trace)
            .await?
            .document
            .ok_or_else(|| DocumentsError::not_found("document", document))
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
