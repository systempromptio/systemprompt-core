//! The images a live checkpoint may place, and a materialise that never
//! wedges a session on one.
//!
//! An uploaded image is placed when its inserting editor or any editor of
//! the session uploaded it ([`ImageOwners`]), or when the revision the
//! checkpoint builds on already holds it (a docx holds a saved upload as
//! `media/sp-<hash>.<ext>`). Any other image is degraded to a placeholder
//! paragraph, and the placeholder is written back into the live document as
//! an ordinary update, so every editor sees it and the next checkpoint is
//! stable. The admitted bytes are read once per checkpoint and serve both
//! the unchanged check and the write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, DocumentFormat};
use systemprompt_provider_contracts::documents::CodecError;

use super::apply::update_event;
use super::document::{ModelError, materialise};
use super::entry::CollabEntry;
use super::image_owners::ImageOwners;
use super::{CollabError, CollabService};
use crate::documents::engine::ModelView;
use crate::documents::model::{BlockKind, Media, RichText, media_refs};
use crate::documents::service::held_media_refs;

/// A live model ready to write: images it may not place are placeholders.
pub(super) struct LiveModel {
    pub(super) model: ModelView,
    pub(super) media: Media,
    held: Vec<String>,
    pub(super) degraded: bool,
}

impl CollabService {
    pub(super) async fn admit_live(
        &self,
        format: DocumentFormat,
        owners: &ImageOwners,
        base_bytes: &[u8],
        model: ModelView,
    ) -> LiveModel {
        let mut rich = match model {
            ModelView::RichText(rich) => rich,
            other => {
                return LiveModel {
                    model: other,
                    media: Media::new(),
                    held: Vec::new(),
                    degraded: false,
                };
            },
        };
        let held = held_media_refs(format, base_bytes);
        let owners_of = |reference: &str| owners.owners(reference);
        let admitted = self
            .documents
            .admit_media(self.uploads.as_deref(), &owners_of, media_refs(&rich), &held)
            .await;
        for refused in &admitted.refused {
            tracing::warn!(
                %format,
                blob = %refused,
                "live checkpoint left out an image no editor of the session may place"
            );
        }
        let degraded = placeholders(&mut rich, &admitted.refused);
        LiveModel {
            model: ModelView::RichText(rich),
            media: admitted.media,
            held,
            degraded,
        }
    }

    /// Writes a degraded model back into the live document as an update
    /// every editor receives, so the placeholder is what they see.
    pub(super) async fn write_back(
        &self,
        entry: &mut CollabEntry,
        who: &AuthenticatedUser,
        model: &ModelView,
        trace: &TraceId,
    ) -> Result<(), CollabError> {
        let Some(update) = entry.doc.update_to_model(model)? else {
            return Ok(());
        };
        let seq = self.log_update(entry, &update, &who.id).await?;
        let document = self
            .authorized(who, &entry.session.document_id, DocumentAction::View, trace)
            .await?;
        self.broadcast(
            &document,
            update_event(&document, &entry.session.id, seq, &update, &who.id),
        )
        .await;
        Ok(())
    }
}

const PLACEHOLDER: &str = "[image not available]";

/// Replaces each image block whose blob is in `refused` with a paragraph
/// saying so, keeping its id, at any depth; answers whether any changed.
fn placeholders(model: &mut RichText, refused: &[String]) -> bool {
    if refused.is_empty() {
        return false;
    }
    let mut changed = false;
    for block in &mut model.blocks {
        if let BlockKind::Image { blob, .. } = &block.kind
            && refused.contains(blob)
        {
            block.kind = BlockKind::Paragraph;
            block.text = PLACEHOLDER.to_owned();
            block.marks.clear();
            changed = true;
            continue;
        }
        for nested in block.kind.nested_mut() {
            changed |= placeholders(nested, refused);
        }
    }
    for part in model.running_mut() {
        changed |= placeholders(part, refused);
    }
    changed
}

/// A live materialise that never wedges a session on an image: when the
/// write fails to resolve an image, the images that were neither admitted
/// nor held become placeholders and the write is retried once. Any other
/// failure is returned as it is.
pub(super) fn materialise_or_strip(
    format: DocumentFormat,
    base: &[u8],
    live: &LiveModel,
) -> Result<Vec<u8>, ModelError> {
    let error = match materialise(format, base, &live.model, &live.media) {
        Ok(bytes) => return Ok(bytes),
        Err(error) => error,
    };
    let image_error = matches!(
        &error,
        ModelError::Codec(CodecError::Refused { detail }) if detail.starts_with("image ")
    );
    let ModelView::RichText(rich) = &live.model else {
        return Err(error);
    };
    let unresolved: Vec<String> = media_refs(rich)
        .into_iter()
        .filter(|reference| !live.media.contains_key(reference) && !live.held.contains(reference))
        .collect();
    if !image_error || unresolved.is_empty() {
        return Err(error);
    }
    tracing::warn!(%format, %error, "live checkpoint retried without its unresolved images");
    let mut stripped = rich.clone();
    placeholders(&mut stripped, &unresolved);
    materialise(format, base, &ModelView::RichText(stripped), &live.media)
}
