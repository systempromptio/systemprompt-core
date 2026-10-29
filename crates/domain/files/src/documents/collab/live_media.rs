//! The images a live checkpoint may place, and a materialise that never
//! wedges a session on one.
//!
//! An uploaded image is placed when an editor the update log records as
//! one of its inserters uploaded it ([`ImageOwners`]), or when the revision the
//! checkpoint builds on already holds it (a docx part whose bytes hash to
//! the reference). Any other image is saved as a placeholder paragraph in
//! the checkpoint's bytes only: the live document keeps the image, so a
//! wrong refusal recovers on the next checkpoint, and the editors get a
//! `document.collab.degraded` event. The admitted bytes are read once per
//! checkpoint and serve both the unchanged check and the write. The
//! degraded event is announced under the checkpoint's actor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, DocumentCollabDegradedEvent, DocumentFormat};
use systemprompt_provider_contracts::documents::CodecError;

use super::binding::checkpoint_actor;
use super::document::{ModelError, materialise, unchanged_from};
use super::entry::CollabEntry;
use super::image_owners::ImageOwners;
use super::{CollabError, CollabService};
use crate::documents::agents::{DocumentActor, act_as};
use crate::documents::engine::ModelView;
use crate::documents::model::{BlockKind, Media, RichText, media_refs};
use crate::documents::models::DocumentEditSession;
use crate::documents::service::held_media_refs;

pub(super) struct LiveModel {
    pub(super) model: ModelView,
    pub(super) media: Media,
    held: Vec<String>,
    pub(super) refused: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct LiveState {
    pub(super) format: DocumentFormat,
    pub(super) session: DocumentEditSession,
    pub(super) owners: ImageOwners,
    pub(super) model: ModelView,
    pub(super) actor: DocumentActor,
}

impl LiveState {
    pub(super) fn of(entry: &CollabEntry) -> Result<Self, CollabError> {
        Ok(Self {
            format: entry.format,
            session: entry.session.clone(),
            owners: entry.images.clone(),
            model: entry.doc.export_model()?,
            actor: checkpoint_actor(entry),
        })
    }
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
        let owners_of = |reference: &str| owners.inserters(reference);
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

    pub(super) async fn changed_live(
        &self,
        state: LiveState,
        who: &AuthenticatedUser,
        base_bytes: &[u8],
        trace: &TraceId,
    ) -> Option<LiveModel> {
        let live = self
            .admit_live(state.format, &state.owners, base_bytes, state.model)
            .await;
        if !live.refused.is_empty()
            && let Err(error) = act_as(
                state.actor,
                self.notify_degraded(&state.session, who, live.refused.clone(), trace),
            )
            .await
        {
            tracing::warn!(%error, "degraded checkpoint could not be announced");
        }
        let unchanged = unchanged_from(state.format, base_bytes, &live.model, &live.media)
            .inspect_err(|error| {
                tracing::warn!(%error, "live checkpoint could not compare with its base");
            })
            .unwrap_or(false);
        (!unchanged).then_some(live)
    }

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
            PLACEHOLDER.clone_into(&mut block.text);
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
