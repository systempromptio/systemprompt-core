//! An image a document's head holds, read for a viewer.
//!
//! Only what the head already holds is served: a `docx-media:` part or a
//! saved upload inside a `docx`, a picture part under `ppt/media/` (`pptx`)
//! or `Pictures/` (`odp`), or, for formats that link their images,
//! an uploaded blob the head's model names and that is admitted to this
//! document (a `document_media` row, see `media_admission`). A reference is
//! not a capability, so anything else is not found, and the validator a
//! viewer revalidates with ([`media_validator`]) is bound to the document
//! rather than disclosing the blob's hash.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ContentHash, DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, DocumentFormat};
use systemprompt_provider_contracts::documents::content_hash;

use super::DocumentService;
use super::media::held_media_refs;
use crate::documents::error::DocumentsError;
use crate::documents::model::MediaBlob;
use crate::documents::models::Document;

#[must_use]
pub fn media_validator(document: &DocumentId, blob: &MediaBlob) -> ContentHash {
    let mut keyed = format!("document-media\0{document}\0").into_bytes();
    keyed.extend_from_slice(content_hash(&blob.bytes).as_str().as_bytes());
    content_hash(&keyed)
}

impl DocumentService {
    pub async fn held_image(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        reference: &str,
        trace: &TraceId,
    ) -> Result<MediaBlob, DocumentsError> {
        let document = self
            .authorized_document(who, document, DocumentAction::View, trace)
            .await?;
        let head = self.resolve_revision(&document, None).await?;
        let bytes = self.revision_bytes(&head).await?;
        self.image_in_revision(&document, &bytes, reference)
            .await?
            .ok_or_else(|| DocumentsError::not_found("image", reference))
    }

    pub(super) async fn image_in_revision(
        &self,
        document: &Document,
        bytes: &[u8],
        reference: &str,
    ) -> Result<Option<MediaBlob>, DocumentsError> {
        let format = document.format;
        let limit = self.config.max_upload_bytes;
        if format == DocumentFormat::Docx {
            return Ok(in_package(bytes, reference, limit));
        }
        if let Some(prefix) = media_folder(format) {
            let part = media_part(bytes, prefix, reference, limit)
                .or_else(|| media_part_by_stem(bytes, prefix, reference, limit));
            let found = part.and_then(|content| {
                let mime = sniff(&content)?.to_owned();
                Some(MediaBlob {
                    bytes: content,
                    mime,
                })
            });
            return Ok(found);
        }
        let wanted = [reference.to_owned()];
        if !held_media_refs(format, bytes, &wanted, limit).contains(&wanted[0]) {
            return Ok(None);
        }
        let Ok(hash) = ContentHash::try_new(reference) else {
            return Ok(None);
        };
        let Some(content) = self.admitted_bytes(&document.id, &hash, limit).await? else {
            return Ok(None);
        };
        Ok(sniff(&content).map(|mime| MediaBlob {
            mime: mime.to_owned(),
            bytes: content,
        }))
    }
}

#[cfg(feature = "office")]
fn in_package(bytes: &[u8], reference: &str, limit: u64) -> Option<MediaBlob> {
    let (mime, bytes) = crate::documents::codecs::docx::held_image(bytes, reference, limit)?;
    Some(MediaBlob { bytes, mime })
}

impl DocumentService {
    async fn admitted_bytes(
        &self,
        document: &DocumentId,
        hash: &ContentHash,
        limit: u64,
    ) -> Result<Option<Vec<u8>>, DocumentsError> {
        if !self.repos.media.is_admitted(document, hash).await? {
            return Ok(None);
        }
        Ok(Some(self.blobs.read_all_primary(hash, limit).await?))
    }

    pub(super) async fn revision_image(
        &self,
        document: &Document,
        bytes: &[u8],
        reference: &str,
        budget: u64,
    ) -> Option<Vec<u8>> {
        let format = document.format;
        let limit = self.config.max_upload_bytes.min(budget);
        if format == DocumentFormat::Docx {
            if let Some(blob) = in_package(bytes, reference, limit) {
                return Some(blob.bytes);
            }
        } else if let Some(prefix) =
            media_folder(format).or_else(|| (format == DocumentFormat::Odt).then_some("Pictures/"))
            && let Some(content) = media_part(bytes, prefix, reference, limit)
        {
            return Some(content);
        }
        let hash = ContentHash::try_new(reference).ok()?;
        self.admitted_bytes(&document.id, &hash, limit)
            .await
            .ok()
            .flatten()
    }
}

const fn media_folder(format: DocumentFormat) -> Option<&'static str> {
    match format {
        DocumentFormat::Pptx => Some("ppt/media/"),
        DocumentFormat::Odp => Some("Pictures/"),
        _ => None,
    }
}

#[cfg(feature = "office")]
pub(super) fn media_part(
    bytes: &[u8],
    prefix: &str,
    reference: &str,
    limit: u64,
) -> Option<Vec<u8>> {
    crate::documents::codecs::media_part(bytes, prefix, reference, limit)
}

#[cfg(not(feature = "office"))]
pub(super) const fn media_part(
    _bytes: &[u8],
    _prefix: &str,
    _reference: &str,
    _limit: u64,
) -> Option<Vec<u8>> {
    None
}

#[cfg(feature = "office")]
fn media_part_by_stem(bytes: &[u8], prefix: &str, stem: &str, limit: u64) -> Option<Vec<u8>> {
    crate::documents::codecs::media_part_by_stem(bytes, prefix, stem, limit)
}

#[cfg(not(feature = "office"))]
const fn media_part_by_stem(
    _bytes: &[u8],
    _prefix: &str,
    _stem: &str,
    _limit: u64,
) -> Option<Vec<u8>> {
    None
}

#[cfg(not(feature = "office"))]
const fn in_package(_bytes: &[u8], _reference: &str, _limit: u64) -> Option<MediaBlob> {
    None
}

fn sniff(bytes: &[u8]) -> Option<&'static str> {
    [
        (&b"\x89PNG"[..], "image/png"),
        (&b"GIF8"[..], "image/gif"),
        (&[0xFF, 0xD8][..], "image/jpeg"),
        (&b"RIFF"[..], "image/webp"),
    ]
    .into_iter()
    .find_map(|(magic, mime)| bytes.starts_with(magic).then_some(mime))
}
