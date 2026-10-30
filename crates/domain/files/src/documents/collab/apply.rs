//! Applying a client's engine update: validate, log, import, broadcast.
//!
//! An update's encoded header is checked first, before the engine imports
//! anything: the engine refuses an oversized, non-incremental or
//! inconsistent blob, or one carrying operations of a peer the server did
//! not assign to the caller's joins. That check is the crash guard: the
//! release profile aborts on panic, so the blocking thread the update is
//! then applied on (a fork of the replica's live document) isolates CPU
//! time, not panics. The fork's import must add operations only of the
//! caller's peers and must not park operations whose dependencies the
//! session lacks. What the import changed is then checked against
//! the model schema, the limits and the caller's access level
//! ([`validate`]): at edit level only the blocks the import touched (the
//! engine's [`ImportScope::Blocks`]), so the check costs what the update
//! changed; at comment level, or when the engine cannot scope the import,
//! the whole model before and after. Block ids are unique only within that
//! scope here; the checkpoint validates the whole model. An accepted update is
//! logged only on top of the log it was validated against: when another replica
//! logged first, the update is caught up and validated again, a bounded number
//! of times. An update enters a replica's cache only from the log, after the
//! transaction that numbered it has committed, so a failed append leaves the
//! cache as it was.
//!
//! Each accepted update extends its author's web presence and is logged with
//! the actor it ran as and the level it was admitted at; every replica folds
//! those into the session's checkpoint actor as it imports the log: one
//! agent while every update since the last checkpoint came from that agent,
//! a human otherwise.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use systemprompt_identifiers::{DocumentEditSessionId, TraceId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, DocumentAction, DocumentCollabUpdateEvent};

use super::entry::SharedEntry;
use super::lease::SessionKey;
use super::{Applied, ApplyRequest, CollabError, CollabService};
use crate::documents::engine::{EngineDoc, EngineError, ImportScope, ModelView};
use crate::documents::model::{EditLevel, Limits, ModelRejection, RichText, validate};
use crate::documents::models::Document;
use crate::documents::repository::{CollabAppend, UpdateAuthor};

const APPEND_ATTEMPTS: u32 = 4;

struct Candidate<'a> {
    update: &'a [u8],
    level: EditLevel,
    peers: &'a [u64],
    author: UpdateAuthor<'a>,
}

impl CollabService {
    pub async fn apply(
        &self,
        who: &AuthenticatedUser,
        request: ApplyRequest,
        trace: &TraceId,
    ) -> Result<Applied, CollabError> {
        let (document, level) = self
            .granted(who, &request.document_id, DocumentAction::Comment, trace)
            .await?;
        let level = if level >= AccessLevel::Edit {
            EditLevel::Edit
        } else {
            EditLevel::Comment
        };
        let repos = self.documents.repositories();
        let ttl = Duration::from_secs(u64::from(self.documents.config().presence_ttl_secs));
        if !repos
            .collab
            .refresh_editing(&document.id, &who.id, ttl)
            .await?
        {
            return Err(CollabError::NotEditing {
                document_id: document.id,
            });
        }
        let session = self
            .open_matching(&document.id, &request.session_id)
            .await?;
        let engine = self.engine()?;
        let peers = self.peers_of(&session.id, &who.id).await?;
        engine
            .check_update(&request.update, Some(&peers))
            .map_err(|error| match error {
                EngineError::PeerNotAssigned { peer } => CollabError::PeerNotAssigned { peer },
                other => CollabError::InvalidUpdate(other),
            })?;
        let shared = self.entry(&document, &session).await?;
        let missing = {
            let entry = self.caught_up(&shared).await?;
            request
                .version
                .as_deref()
                .map(|known| entry.doc.missing_since(known))
                .transpose()
                .map_err(CollabError::InvalidVersion)?
                .flatten()
        };
        let actor = self.documents.access().actor();
        let candidate = Candidate {
            update: &request.update,
            level,
            peers: &peers,
            author: UpdateAuthor {
                user: &who.id,
                actor: &actor,
                level,
            },
        };
        let seq = self.append_validated(&shared, &candidate).await?;
        let version = self.record_and_snapshot(&shared, who, &session.id).await;
        self.broadcast(
            &document,
            update_event(&document, &session.id, seq, &request.update, &who.id),
        )
        .await;
        Ok(Applied {
            seq,
            version,
            missing,
        })
    }

    async fn record_and_snapshot(
        &self,
        shared: &SharedEntry,
        who: &AuthenticatedUser,
        session_id: &DocumentEditSessionId,
    ) -> Vec<u8> {
        let (version, snapshot_due) = {
            let mut entry = shared.lock().await;
            entry.record_activity(who);
            let due =
                !entry.snapshotting && entry.unsnapshotted >= self.tuning.snapshot_after_updates;
            (entry.doc.version(), due)
        };
        if snapshot_due && let Err(error) = self.snapshot(shared).await {
            tracing::warn!(
                session_id = %session_id,
                error = %error,
                "Edit session snapshot failed; the timer retries it"
            );
        }
        version
    }

    async fn append_validated(
        &self,
        shared: &SharedEntry,
        candidate: &Candidate<'_>,
    ) -> Result<i64, CollabError> {
        let collab = &self.documents.repositories().collab;
        let key = SessionKey::of(&*shared.lock().await);
        for _ in 0..APPEND_ATTEMPTS {
            let (draft, on_seq) = {
                let entry = self.caught_up(shared).await?;
                (entry.doc.fork(), entry.last_seq)
            };
            check_on_fork(draft, candidate).await?;
            match collab
                .append_on(&key.session, on_seq, candidate.update, candidate.author)
                .await?
            {
                CollabAppend::Appended(seq) => {
                    self.refresh(shared).await?;
                    return Ok(seq);
                },
                CollabAppend::Behind => {},
                CollabAppend::Closed => return Err(key.closed()),
            }
        }
        Err(CollabError::UpdateContended {
            document_id: key.document,
        })
    }

    pub(super) async fn log_update(
        &self,
        shared: &SharedEntry,
        key: &SessionKey,
        update: &[u8],
        author: UpdateAuthor<'_>,
    ) -> Result<i64, CollabError> {
        let appended = self
            .documents
            .repositories()
            .collab
            .append(&key.session, update, author)
            .await;
        let seq = match appended {
            Ok(Some(seq)) => seq,
            Ok(None) => return Err(key.closed()),
            Err(error) => {
                if let Err(caught) = self.caught_up(shared).await {
                    tracing::warn!(
                        session_id = %key.session,
                        error = %caught,
                        "Live session not caught up after a failed log write; reopening it"
                    );
                    self.cache_remove(&key.document);
                }
                return Err(error.into());
            },
        };
        self.refresh(shared).await?;
        Ok(seq)
    }

    pub(super) async fn broadcast(&self, document: &Document, event: DocumentCollabUpdateEvent) {
        if let Err(error) = self.documents.events().collab_update(&event).await {
            tracing::warn!(
                document_id = %document.id,
                seq = event.seq,
                error = %error,
                "Collab update event not delivered"
            );
        }
    }
}

async fn check_on_fork(
    mut draft: Box<dyn EngineDoc>,
    candidate: &Candidate<'_>,
) -> Result<(), CollabError> {
    let update = candidate.update.to_vec();
    let scoped = candidate.level == EditLevel::Edit;
    let imported =
        tokio::task::spawn_blocking(move || draft.apply_update_scoped(&update, scoped)).await;
    let scoped = match imported {
        Ok(result) => result.map_err(CollabError::InvalidUpdate)?,
        Err(error) => {
            tracing::warn!(
                error = %error,
                panicked = error.is_panic(),
                "Collab update import did not complete on its fork"
            );
            return Err(CollabError::InvalidUpdate(EngineError::State(Box::new(
                error,
            ))));
        },
    };
    let import = scoped.import;
    if import.pending {
        return Err(CollabError::MissingDependencies);
    }
    if let Some(peer) = import
        .peers
        .iter()
        .find(|peer| !candidate.peers.contains(peer))
    {
        return Err(CollabError::PeerNotAssigned { peer: *peer });
    }
    match scoped.scope {
        ImportScope::Blocks(touched) => validate(
            &RichText::default(),
            &touched,
            EditLevel::Edit,
            &Limits::default(),
        )
        .map_err(CollabError::UpdateRejected),
        ImportScope::Full { before, after } => check_model(&before, after, candidate.level),
    }
}

fn check_model(before: &ModelView, after: ModelView, level: EditLevel) -> Result<(), CollabError> {
    match after {
        ModelView::RichText(after) => {
            let empty = RichText::default();
            let before = match before {
                ModelView::RichText(before) => before,
                _ => &empty,
            };
            validate(before, &after, level, &Limits::default()).map_err(CollabError::UpdateRejected)
        },
        after if level == EditLevel::Comment && after != *before => Err(
            CollabError::UpdateRejected(ModelRejection::BeyondCommentLevel),
        ),
        ModelView::Body(_) => Ok(()),
        #[cfg(feature = "collab-loro")]
        ModelView::Sheet(_) | ModelView::Deck(_) => Ok(()),
    }
}

pub(super) fn update_event(
    document: &Document,
    session: &DocumentEditSessionId,
    seq: i64,
    update: &[u8],
    author: &UserId,
) -> DocumentCollabUpdateEvent {
    DocumentCollabUpdateEvent {
        document_id: document.id.clone(),
        edit_session_id: session.clone(),
        seq,
        update_b64: STANDARD.encode(update),
        author_id: author.clone(),
    }
}
