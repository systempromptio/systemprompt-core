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
use crate::documents::model::{Media, MediaBlob, media_refs};
use crate::uploads::UploadService;

/// The images a write may place, and the references it may not.
#[derive(Debug, Default)]
pub struct AdmittedMedia {
    pub media: Media,
    pub refused: Vec<String>,
}

/// The uploaded-image references a revision's bytes already hold.
#[must_use]
pub fn held_media_refs(format: DocumentFormat, bytes: &[u8]) -> Vec<String> {
    import_rich_text(format, bytes)
        .map(|model| media_refs(&model))
        .unwrap_or_default()
}

impl DocumentService {
    pub async fn admit_media(
        &self,
        uploads: Option<&UploadService>,
        owner: &UserId,
        refs: Vec<String>,
        held: &[String],
    ) -> AdmittedMedia {
        let mut admitted = AdmittedMedia::default();
        for reference in refs {
            let Ok(hash) = ContentHash::try_new(&reference) else {
                admitted.refused.push(reference);
                continue;
            };
            let owned = match uploads {
                Some(uploads) => uploads.require_owned_blob(owner, &hash).await.ok(),
                None => None,
            };
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
