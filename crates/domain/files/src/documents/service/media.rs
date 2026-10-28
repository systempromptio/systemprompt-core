//! Which uploaded images a write may place, and their bytes.
//!
//! A blob reference is not a capability: it is visible in the model to
//! every viewer and outlives a revoked share. An image is admitted only
//! when (a) one of its owners completed the upload that produced it, and
//! its bytes are then read from the blob store, or (b) the document
//! already holds it (`held`, from the revision the write builds on), and
//! it is then placed from the document's own bytes, never the store. Everything
//! else is returned as refused, for the caller to refuse (an agent write) or
//! degrade (a live checkpoint).
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

/// The uploaded-image references a revision's bytes already hold. In
/// Markdown and HTML that is a reference the text names. A docx holds a
/// saved upload as its picture part `media/sp-<hash>.<ext>`, which counts
/// only when the part's bytes hash to `<hash>`: a part named for another
/// user's upload never stands in for it. A held image is placed from the
/// document's own bytes and never read from the blob store.
/// Only the references in `wanted` (what the write places) are looked
/// for, and a picture part over `limit` bytes is never read.
#[must_use]
pub fn held_media_refs(
    format: DocumentFormat,
    bytes: &[u8],
    wanted: &[String],
    limit: u64,
) -> Vec<String> {
    let mut held = import_rich_text(format, bytes)
        .map(|model| media_refs(&model))
        .unwrap_or_default();
    if format == DocumentFormat::Docx {
        held.extend(saved_uploads(bytes, wanted, limit));
    }
    held.sort();
    held.dedup();
    held
}

#[cfg(feature = "office")]
fn saved_uploads(bytes: &[u8], wanted: &[String], limit: u64) -> Vec<String> {
    crate::documents::codecs::docx::saved_upload_hashes(bytes, wanted, limit)
}

#[cfg(not(feature = "office"))]
const fn saved_uploads(_bytes: &[u8], _wanted: &[String], _limit: u64) -> Vec<String> {
    Vec::new()
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
            if held.contains(&reference) {
                continue;
            }
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
            let Some(origin) = owned else {
                admitted.refused.push(reference);
                continue;
            };
            let mime = origin.mime;
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
