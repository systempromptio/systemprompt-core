//! The live session's current content, for reads that should see what the
//! open tabs see: the bytes its model materialises to on its base revision,
//! and the model itself, whose block ids are the ids a read reports.
//!
//! Only a session this replica holds is read, so a read never opens or
//! loads one; any other read answers from the head.
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

/// A live session's base revision, its materialised bytes and its model.
#[derive(Debug, Clone)]
pub(crate) struct LiveView {
    pub base: DocumentRevisionId,
    pub bytes: Vec<u8>,
    pub model: RichText,
}

impl CollabService {
    pub(crate) async fn live_view(
        &self,
        document: &Document,
    ) -> Result<Option<LiveView>, CollabError> {
        let Some(shared) = self.cached(&document.id) else {
            return Ok(None);
        };
        let Some(body) = self.fork_body(&shared).await? else {
            return Ok(None);
        };
        if !matches!(body.state.model, ModelView::RichText(_)) {
            return Ok(None);
        }
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
        let ModelView::RichText(model) = live.model else {
            return Ok(None);
        };
        Ok(Some(LiveView {
            base: body.base,
            bytes,
            model,
        }))
    }
}
