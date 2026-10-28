//! The images a live checkpoint may place, and a materialise that never
//! wedges a session on one.
//!
//! An uploaded image is placed when the editor the update log records as
//! its inserter uploaded it ([`ImageOwners`]), or when the revision the
//! checkpoint builds on already holds it (a docx part whose bytes hash to
//! the reference). Any other image is saved as a placeholder paragraph in
//! the checkpoint's bytes only: the live document keeps the image, so a
//! wrong refusal recovers on the next checkpoint, and the editors get a
//! `document.collab.degraded` event. The admitted bytes are read once per
//! checkpoint and serve both the unchanged check and the write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, DocumentCollabDegradedEvent, DocumentFormat};
use systemprompt_provider_contracts::documents::CodecError;

use super::document::{ModelError, materialise};
use crate::documents::models::DocumentEditSession;
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
    /// The images saved as placeholders; the live document still holds
    /// them, so a later checkpoint saves them once they are admitted.
    pub(super) refused: Vec<String>,
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
                    refused: Vec::new(),
                };
            },
        };
        let wanted = media_refs(&rich);
        let limit = self.documents.config().max_upload_bytes;
        let held = held_media_refs(format, base_bytes, &wanted, limit);
        let owners_of =
            |reference: &str| owners.owner(reference).cloned().into_iter().collect::<Vec<_>>();
        let admitted = self
            .documents
            .admit_media(self.uploads.as_deref(), &owners_of, wanted, &held)
            .await;
        for refused in &admitted.refused {
            tracing::warn!(
                %format,
                blob = %refused,
                "live checkpoint saved a placeholder for an image its inserter may not place"
            );
        }
        placeholders(&mut rich, &admitted.refused);
        LiveModel {
            model: ModelView::RichText(rich),
            media: admitted.media,
            held,
            refused: admitted.refused,
        }
    }

    /// Tells the session's editors which images a checkpoint saved as
    /// placeholders; the live document itself is left as it is.
    pub(super) async fn notify_degraded(
        &self,
        session: &DocumentEditSession,
        who: &AuthenticatedUser,
        images: Vec<String>,
        trace: &TraceId,
    ) -> Result<(), CollabError> {
        let document = self
            .authorized(who, &session.document_id, DocumentAction::View, trace)
            .await?;
        let to = self.recipients(&document).await;
        let event = DocumentCollabDegradedEvent {
            document_id: document.id.clone(),
            edit_session_id: session.id.clone(),
            images,
        };
        self.documents.events().collab_degraded(&to, &event).await?;
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
