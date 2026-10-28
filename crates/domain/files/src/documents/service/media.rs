//! Which uploaded images a write may place, and their bytes.
//!
//! A blob reference is not a capability: it is visible in the model to
//! every viewer and outlives a revoked share. An image is admitted only
//! when (a) the writing user completed the upload that produced it, or
//! (b) the document already references it (`held`, read from the
//! revision the write builds on). Everything else is returned as refused,
//! for the caller to refuse (an agent write) or degrade (a live
//! checkpoint).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ContentHash, UserId};
use systemprompt_models::documents::DocumentFormat;

use super::DocumentService;
use crate::documents::model::formats::import_rich_text;
use crate::documents::model::{BlockKind, Media, MediaBlob, RichText, media_refs};
use crate::uploads::UploadService;

/// The images a write may place, and the references it may not.
#[derive(Debug, Default)]
pub struct AdmittedMedia {
    pub media: Media,
    pub refused: Vec<String>,
}

/// The uploaded-image references a revision's bytes already hold. A docx
/// holds a placed upload as its picture part `media/sp-<hash>.<ext>`, which
/// counts as holding `<hash>`, so an image a checkpoint already saved stays
/// placeable.
#[must_use]
pub fn held_media_refs(format: DocumentFormat, bytes: &[u8]) -> Vec<String> {
    let Ok(model) = import_rich_text(format, bytes) else {
        return Vec::new();
    };
    let mut held = media_refs(&model);
    saved_uploads(&model, &mut held);
    held.sort();
    held.dedup();
    held
}

fn saved_uploads(model: &RichText, into: &mut Vec<String>) {
    for block in &model.blocks {
        if let BlockKind::Image { blob, .. } = &block.kind
            && let Some(hash) = saved_hash(blob)
        {
            into.push(hash);
        }
        for nested in block.kind.nested() {
            saved_uploads(nested, into);
        }
    }
    for part in model.running() {
        saved_uploads(part, into);
    }
}

/// `<hash>` from `docx-media:media/sp-<hash>.<ext>`.
fn saved_hash(blob: &str) -> Option<String> {
    let file = blob.strip_prefix("docx-media:media/sp-")?;
    let hash = file.split('.').next()?;
    (hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit())).then(|| hash.to_owned())
}

impl DocumentService {
    /// `owners` answers, per reference, the users whose upload may place
    /// it: the writer for an agent write, the inserting editor and every
    /// session editor for a live checkpoint.
    pub async fn admit_media(
        &self,
        uploads: Option<&UploadService>,
        owners: &(dyn Fn(&str) -> Vec<UserId> + Sync),
        refs: Vec<String>,
        held: &[String],
    ) -> AdmittedMedia {
        let mut admitted = AdmittedMedia::default();
        for reference in refs {
            let Ok(hash) = ContentHash::try_new(&reference) else {
                admitted.refused.push(reference);
                continue;
            };
            let mut owned = None;
            if let Some(uploads) = uploads {
                for owner in owners(&reference) {
                    if let Ok(origin) = uploads.require_owned_blob(&owner, &hash).await {
                        owned = Some(origin);
                        break;
                    }
                }
            }
            let mime = match owned {
                Some(origin) => origin.mime,
                None if held.contains(&reference) => "application/octet-stream".to_owned(),
                None => {
                    admitted.refused.push(reference);
                    continue;
                },
            };
            match self.media_bytes(&hash).await {
                Ok(bytes) => {
                    admitted.media.insert(reference, MediaBlob { bytes, mime });
                },
                Err(error) => {
                    tracing::warn!(blob = %reference, error = %error, "image bytes unreadable");
                    admitted.refused.push(reference);
                },
            }
        }
        admitted
    }
}
