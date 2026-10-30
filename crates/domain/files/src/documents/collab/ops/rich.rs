//! Text-document ops in a live session.
//!
//! The live model is materialised onto the session's base revision, the
//! batch is applied to those bytes under node guards checked against the
//! live tree (`DocumentService::apply_ops_live`), and the result is imported
//! back as a `RichText` model with the live block ids carried over. The
//! engine computes the update from the live document to that model on a
//! fork, written under the caller's per-session peer, checked like a
//! client update and logged, imported and broadcast as the session's next
//! update. A batch that changes nothing is answered without an update.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::PoisonError;

use systemprompt_models::auth::AuthenticatedUser;

use crate::documents::collab::apply::update_event;
use crate::documents::collab::document::import_rich_text;
use crate::documents::collab::error::model_error;
use crate::documents::collab::live_media::materialise_or_strip;
use crate::documents::collab::{CollabError, CollabService};
use crate::documents::engine::ModelView;
use crate::documents::model::{EditLevel, carry_ids};
use crate::documents::models::Document;
use crate::documents::repository::UpdateAuthor;
use crate::documents::service::{LiveEdit, LiveEditRequest, LiveOps};

impl CollabService {
    pub(super) async fn edit_rich(
        &self,
        who: &AuthenticatedUser,
        document: &Document,
        request: LiveEditRequest,
    ) -> Result<Option<LiveEdit>, CollabError> {
        let repos = self.documents.repositories();
        let Some(session) = repos.edit_sessions.find_open(&document.id).await? else {
            return Ok(None);
        };
        let shared = self.entry(document, &session).await?;
        let Some(body) = self.fork_body(&shared).await? else {
            return Ok(None);
        };
        let ModelView::RichText(current) = &body.state.model else {
            return Ok(None);
        };
        let format = body.state.format;
        let limit = self.documents.config().max_upload_bytes;
        let base_bytes = self.revision_bytes(&body.base, limit).await?;
        let live = self
            .admit_live(
                format,
                &body.state.owners,
                &base_bytes,
                body.state.model.clone(),
            )
            .await;
        let live_bytes =
            materialise_or_strip(format, &base_bytes, &live).map_err(model_error(&document.id))?;
        let applied_ops = request.ops.len();
        let applied = self
            .documents
            .apply_ops_live(LiveOps {
                document,
                base: &body.base,
                live: live_bytes,
                ops: request.ops,
                media: request.media,
            })
            .await?;
        let mut next =
            import_rich_text(format, &applied.bytes).map_err(model_error(&document.id))?;
        carry_ids(current, &mut next);
        let actor = self.documents.access().actor();
        let peer = self.actor_peer(&session.id, &who.id, &actor).await?;
        let update = body
            .fork
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .update_to_model_as(&ModelView::RichText(next), peer)?;
        let mut edit = LiveEdit {
            session_id: session.id.clone(),
            seq: None,
            base_revision: body.base.clone(),
            before: applied.before,
            after: applied.after,
            applied: applied_ops,
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
        let seq = self.log_update(&shared, &body.key, &update, author).await?;
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
