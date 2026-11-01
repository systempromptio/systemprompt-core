//! The live session's current content, for reads that should see what the
//! open tabs see: the bytes its model materialises to on its base revision,
//! and, for a text document, the model itself, whose block ids are the ids
//! a read reports. A workbook or deck session materialises its sheet or
//! deck model the same way, so a read sees its live cells and slides.
//!
//! Only a session this replica holds is read, so a read never opens or
//! loads one; any other read answers from the head, as does a read of a
//! held session that is no longer the document's open one. A held session
//! whose row checkpointed since (on the replica that owns it) takes the
//! row first, so the read names the newest checkpoint as its base.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentRevisionId;

use crate::documents::collab::error::model_error;
use crate::documents::collab::live_media::materialise_or_strip;
use crate::documents::collab::{CollabError, CollabService};
use crate::documents::engine::ModelView;
use crate::documents::model::RichText;
use crate::documents::models::Document;

#[derive(Debug, Clone)]
pub(crate) struct LiveView {
    pub base: DocumentRevisionId,
    pub bytes: Vec<u8>,
    pub model: Option<RichText>,
}

impl CollabService {
    pub(crate) async fn live_view(
        &self,
        document: &Document,
    ) -> Result<Option<LiveView>, CollabError> {
        let Some(shared) = self.cached(&document.id) else {
            return Ok(None);
        };
        let open = self
            .documents
            .repositories()
            .edit_sessions
            .find_open(&document.id)
            .await?;
        let Some(open) = open else {
            return Ok(None);
        };
        {
            let mut entry = shared.lock().await;
            if entry.session.id != open.id {
                return Ok(None);
            }
            if entry.session.checkpoint_revision_id != open.checkpoint_revision_id {
                entry.session = open;
            }
        }
        let Some(body) = self.fork_body(&shared).await? else {
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
        let bytes =
            materialise_or_strip(format, &base_bytes, &live).map_err(model_error(&document.id))?;
        let model = match live.model {
            ModelView::RichText(model) => Some(model),
            _ => None,
        };
        Ok(Some(LiveView {
            base: body.base,
            bytes,
            model,
        }))
    }
}
