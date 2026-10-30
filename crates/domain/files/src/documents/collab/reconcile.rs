//! Writing a merged or head document back into a live edit session: the
//! update is computed on the checkpoint's fork, logged as the session's next
//! update and broadcast to the session's participants.
//!
//! A write-back carries content a revision already holds, so it is logged
//! under the `collab-write-back` system actor rather than as anyone's edit:
//! choosing the checkpoint author and folding the checkpoint actor skip it,
//! and the participant who happened to trigger it never becomes an author.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::PoisonError;

use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentAction;

use super::apply::update_event;
use super::document::{import, import_rich_text};
use super::entry::SharedEntry;
use super::error::{model_error, utf8};
use super::model::body_of;
use super::persist_commit::Body;
use super::{CollabError, CollabService};
use crate::documents::agents::DocumentActor;
use crate::documents::engine::ModelView;
use crate::documents::model::{EditLevel, carry_ids};
use crate::documents::repository::UpdateAuthor;

pub(super) const WRITE_BACK_ACTOR: &str = "collab-write-back";

pub(super) struct WriteBack<'a> {
    pub(super) who: &'a AuthenticatedUser,
    pub(super) level: EditLevel,
    pub(super) merged: &'a [u8],
}

impl CollabService {
    pub(super) async fn reconcile(
        &self,
        shared: &SharedEntry,
        body: &Body,
        back: WriteBack<'_>,
        trace: &TraceId,
    ) -> Result<(), CollabError> {
        let WriteBack { who, level, merged } = back;
        let document_id = &body.key.document;
        let format = body.state.format;
        let target = match &body.state.model {
            ModelView::RichText(live) => {
                let mut next =
                    import_rich_text(format, merged).map_err(model_error(document_id))?;
                carry_ids(live, &mut next);
                ModelView::RichText(next)
            },
            ModelView::Body(_) => {
                ModelView::Body(body_of(format, &utf8(document_id, merged.to_vec())?))
            },
            #[cfg(feature = "collab-loro")]
            ModelView::Sheet(_) | ModelView::Deck(_) => {
                import(format, merged).map_err(model_error(document_id))?
            },
        };
        let update = body
            .fork
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .update_to_model(&target)?;
        let Some(update) = update else {
            return Ok(());
        };
        let actor = DocumentActor::system(WRITE_BACK_ACTOR);
        let author = UpdateAuthor {
            user: &who.id,
            actor: &actor,
            level,
        };
        let seq = self.log_update(shared, &body.key, &update, author).await?;
        let document = self
            .authorized(who, document_id, DocumentAction::View, trace)
            .await?;
        self.broadcast(
            &document,
            update_event(&document, &body.key.session, seq, &update, &who.id),
        )
        .await;
        Ok(())
    }
}
