//! An image a document's head holds, read for a viewer.
//!
//! Only what the head already holds is served: a `docx-media:` part or a
//! saved upload inside a `docx`, or, for formats that link their images,
//! an uploaded blob the head's model names. A reference is not a
//! capability, so anything else is not found.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ContentHash, DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, DocumentFormat};

use super::DocumentService;
use super::media::held_media_refs;
use crate::documents::error::DocumentsError;
use crate::documents::model::MediaBlob;

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
        let limit = self.config.max_upload_bytes;
        let missing = || DocumentsError::not_found("image", reference);
        if document.format == DocumentFormat::Docx {
            return in_package(&bytes, reference, limit).ok_or_else(missing);
        }
        let wanted = [reference.to_owned()];
        if !held_media_refs(document.format, &bytes, &wanted, limit).contains(&wanted[0]) {
            return Err(missing());
        }
        let Ok(hash) = ContentHash::try_new(reference) else {
            return Err(missing());
        };
        let content = self.media_bytes(&hash).await?;
        let mime = sniff(&content).ok_or_else(missing)?;
        Ok(MediaBlob {
            bytes: content,
            mime: mime.to_owned(),
        })
    }
}

#[cfg(feature = "office")]
fn in_package(bytes: &[u8], reference: &str, limit: u64) -> Option<MediaBlob> {
    let (mime, bytes) = crate::documents::codecs::docx::held_image(bytes, reference, limit)?;
    Some(MediaBlob { bytes, mime })
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
