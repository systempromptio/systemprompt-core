//! The images a live checkpoint may place, and a materialise that never
//! wedges a session on one.
//!
//! An uploaded image is placed when the editing user uploaded it or the
//! revision the checkpoint builds on already holds it
//! (`DocumentService::admit_media`); any other image becomes a placeholder
//! paragraph with a warning. The admitted bytes are read once per
//! checkpoint and serve both the unchanged check and the write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentFormat;

use super::CollabService;
use super::document::{ModelError, materialise};
use crate::documents::engine::ModelView;
use crate::documents::model::{BlockKind, Media, RichText, media_refs};
use crate::documents::service::held_media_refs;

impl CollabService {
    /// The live model with every image the editor may not place swapped
    /// for a placeholder paragraph, and the bytes of the images it may: an
    /// image is placed when `who` uploaded it or the base revision already
    /// holds it. A checkpoint never fails on an image; each one left out is
    /// logged.
    pub(super) async fn admit_live(
        &self,
        format: DocumentFormat,
        who: &AuthenticatedUser,
        base_bytes: &[u8],
        model: ModelView,
    ) -> (ModelView, Media) {
        let mut rich = match model {
            ModelView::RichText(rich) => rich,
            other => return (other, Media::new()),
        };
        let held = held_media_refs(format, base_bytes);
        let admitted = self
            .documents
            .admit_media(self.uploads.as_deref(), &who.id, media_refs(&rich), &held)
            .await;
        for refused in &admitted.refused {
            tracing::warn!(
                %format,
                blob = %refused,
                "live checkpoint left out an image the editor may not place"
            );
        }
        placeholders(&mut rich, &admitted.refused);
        (ModelView::RichText(rich), admitted.media)
    }

}

const PLACEHOLDER: &str = "[image not available]";

/// Replaces each image block whose blob is in `refused` with a paragraph
/// saying so, keeping its id, inside table cells too.
fn placeholders(model: &mut RichText, refused: &[String]) {
    if refused.is_empty() {
        return;
    }
    for block in &mut model.blocks {
        match &mut block.kind {
            BlockKind::Image { blob, .. } if refused.contains(blob) => {
                block.kind = BlockKind::Paragraph;
                block.text = PLACEHOLDER.to_owned();
                block.marks.clear();
            },
            BlockKind::Table { rows, .. } => {
                for cell in rows.iter_mut().flatten() {
                    placeholders(cell, refused);
                }
            },
            _ => {},
        }
    }
}

/// A live materialise that never wedges a session on an image: when the
/// write fails, every uploaded image becomes a placeholder and the write is
/// retried once.
pub(super) fn materialise_or_strip(
    format: DocumentFormat,
    base: &[u8],
    model: &ModelView,
    media: &Media,
) -> Result<Vec<u8>, ModelError> {
    match materialise(format, base, model, media) {
        Ok(bytes) => Ok(bytes),
        Err(error) => {
            let ModelView::RichText(rich) = model else {
                return Err(error);
            };
            tracing::warn!(%format, %error, "live checkpoint retried without its uploaded images");
            let mut stripped = rich.clone();
            placeholders(&mut stripped, &media_refs(rich));
            materialise(format, base, &ModelView::RichText(stripped), media)
        },
    }
}
