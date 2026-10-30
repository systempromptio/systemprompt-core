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
//! refusal. A workbook or deck batch addresses the node ids of the
//! session's base revision; they are lowered to the codec paths the
//! translators take, and the paths written are named back by those ids.
//! No node-id projection is built on this path, so the ack's
//! before and after trees are empty; it names the block and cell ids the
//! translator wrote instead, and the caller reads those lines from the live
//! projection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_provider_contracts::documents::{Projection, content_hash};

use crate::documents::collab::apply::update_event;
use crate::documents::collab::lease::SessionKey;
use crate::documents::collab::{CollabError, CollabService};
use crate::documents::engine::{EngineError, OpsUpdate};
use crate::documents::model::EditLevel;
use crate::documents::models::Document;
use crate::documents::nodes::{path_of, to_paths};
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
        let addressed_by_path = !lenient;
        let base_ids = if addressed_by_path {
            Some(self.documents.revision_ids(document, &base).await?)
        } else {
            None
        };
        let ops = match &base_ids {
            Some(ids) => to_paths(request.ops.clone(), ids),
            None => request.ops.clone(),
        };
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
            touched: Vec::new(),
        };
        let Some(OpsUpdate { update, touched }) = update else {
            return Ok(Some(edit));
        };
        edit.touched = match &base_ids {
            Some(ids) => node_ids_of(touched, ids),
            None => touched,
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

fn node_ids_of(paths: Vec<String>, ids: &Projection) -> Vec<String> {
    let by_path: HashMap<&str, &str> = ids
        .nodes()
        .map(|node| (path_of(node), node.id.as_str()))
        .collect();
    paths
        .into_iter()
        .map(|path| {
            by_path
                .get(path.as_str())
                .map_or(path, |id| (*id).to_owned())
        })
        .collect()
}
