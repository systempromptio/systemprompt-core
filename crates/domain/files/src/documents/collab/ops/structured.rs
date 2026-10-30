//! Ops translated by the engine in a live session: sheets, decks, and text
//! documents first of all.
//!
//! The engine translates the batch itself (`EngineDoc::update_from_ops`,
//! which the Loro engine answers with the `sheet`, `deck` and `rich_loro`
//! translators)
//! on a fork of the live document under a peer freshly assigned to the
//! caller. The update is then checked like a client update and logged,
//! imported and broadcast as the session's next update. A `lenient` call
//! (text documents) answers `None` on any translation refusal, so the
//! caller retries the batch through the model, which answers the precise
//! refusal. No node-id projection is built on this path, so the ack's
//! before and after trees are empty and its `written` lines are none.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_provider_contracts::documents::{Projection, content_hash};

use crate::documents::collab::apply::update_event;
use crate::documents::collab::lease::SessionKey;
use crate::documents::collab::{CollabError, CollabService};
use crate::documents::engine::EngineError;
use crate::documents::model::EditLevel;
use crate::documents::models::Document;
use crate::documents::repository::UpdateAuthor;
use crate::documents::service::{LiveEdit, LiveEditRequest};

impl CollabService {
    pub(super) async fn edit_structured(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        request: &LiveEditRequest,
        lenient: bool,
    ) -> Result<Option<LiveEdit>, CollabError> {
        let repos = self.documents.repositories();
        let Some(session) = repos.edit_sessions.find_open(&document.id).await? else {
            return Ok(None);
        };
        let shared = self.entry(document, &session).await?;
        let actor = self.documents.access().actor();
        let peer = self.actor_peer(&session.id, &who.id, &actor).await?;
        let (fork, key, base) = {
            let entry = self.caught_up(&shared).await?;
            let Some(base) = entry.checkpoint_base().cloned() else {
                return Ok(None);
            };
            (entry.doc.fork(), SessionKey::of(&entry), base)
        };
        let ops = request.ops.clone();
        let applied = ops.len();
        let translated = tokio::task::spawn_blocking(move || fork.update_from_ops(&ops, peer))
            .await
            .map_err(|error| CollabError::InvalidUpdate(EngineError::State(Box::new(error))))?;
        let update = match translated {
            Err(EngineError::Unsupported) => return Ok(None),
            Err(_) if lenient => return Ok(None),
            other => other.map_err(CollabError::InvalidUpdate)?,
        };
        let empty = || Projection {
            format: document.format,
            root: Vec::new(),
            content_sha256: content_hash(&[]),
        };
        let mut edit = LiveEdit {
            session_id: session.id.clone(),
            seq: None,
            base_revision: base,
            before: empty(),
            after: empty(),
            applied,
        };
        let Some(update) = update else {
            return Ok(Some(edit));
        };
        self.engine()?
            .check_update(&update, Some(&[peer]))
            .map_err(CollabError::InvalidUpdate)?;
        let author = UpdateAuthor {
            user: &who.id,
            actor: &actor,
            level: EditLevel::Edit,
        };
        let seq = self.log_update(&shared, &key, &update, author).await?;
        shared.lock().await.record_activity(who);
        self.broadcast(
            document,
            update_event(document, &session.id, seq, &update, &who.id),
        )
        .await;
        edit.seq = Some(seq);
        Ok(Some(edit))
    }
}
