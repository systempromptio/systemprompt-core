//! Which uploaded images a revision admits to its document.
//!
//! Every revision a document stores passes through here before it lands, on
//! every writer: the HTTP bytes and ops commits, Drive commits, import,
//! creation from a template, copies, agent and live edits, checkpoints and
//! proposals. The references that need admission are the content hashes the
//! revision's rich text model names and its own package does not carry (a
//! saved upload inside a `docx`, an `odt` picture part). Each one is
//! admitted to the document when:
//!
//! - it is already admitted to this document, or
//! - the committing user completed the upload that produced those bytes, or
//! - the caller vouches for it: the write's [`Media`] carries it, which only
//!   [`DocumentService::admit_media`] fills (from an upload one of the
//!   reference's owners completed), or the copy's source document has it
//!   admitted and the copier may read that source.
//!
//! Knowing a hash admits nothing. A revision naming any other hash is
//! refused whole, whether or not such a blob exists, so the refusal is no
//! existence oracle. An admitted reference whose blob is not stored records
//! nothing, since there is nothing to serve.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ContentHash, UserId};
use systemprompt_models::documents::DocumentFormat;

use super::DocumentService;
use super::ingest::refused;
use super::markdown_export::has_rich_model;
use super::media::saved_uploads;
use super::media_read::media_part;
use super::support::blocking;
use crate::documents::error::DocumentsError;
use crate::documents::model::formats::import_rich_text;
use crate::documents::model::{Media, media_refs};
use crate::documents::models::Document;
use crate::uploads::UploadError;

const REFUSED_CODE: &str = "media_not_admitted";

#[must_use]
pub(super) fn vouched_by(media: &Media) -> Vec<String> {
    media.keys().cloned().collect()
}

impl DocumentService {
    pub(super) async fn admissible_media(
        &self,
        who: &UserId,
        document: &Document,
        bytes: &[u8],
        vouched: &[String],
    ) -> Result<Vec<String>, DocumentsError> {
        let format = document.format;
        if !has_rich_model(format) {
            return Ok(Vec::new());
        }
        let limit = self.config.max_upload_bytes;
        let source = bytes.to_vec();
        let refs = blocking(move || Ok(store_refs(format, &source, limit))).await?;
        if refs.is_empty() {
            return Ok(refs);
        }
        let held = self.repos.media.admitted_among(&document.id, &refs).await?;
        let mut refused_refs = Vec::new();
        for reference in &refs {
            if held.contains(reference) || vouched.contains(reference) {
                continue;
            }
            if !self.uploaded_by(who, reference).await? {
                refused_refs.push(reference.clone());
            }
        }
        if !refused_refs.is_empty() {
            return Err(DocumentsError::ContentRefused {
                code: REFUSED_CODE,
                detail: format!(
                    "the document names {} image(s) that are neither already in it nor uploaded \
                     by you: {}",
                    refused_refs.len(),
                    refused_refs.join(", ")
                ),
            });
        }
        Ok(refs)
    }

    async fn uploaded_by(&self, who: &UserId, reference: &str) -> Result<bool, DocumentsError> {
        let Some(uploads) = self.uploads.get() else {
            return Ok(false);
        };
        let Ok(hash) = ContentHash::try_new(reference) else {
            return Ok(false);
        };
        match uploads.require_owned_blob(who, &hash).await {
            Ok(_) => Ok(true),
            Err(UploadError::BlobNotOwned(_)) => Ok(false),
            Err(error) => Err(refused(error)),
        }
    }
}

fn store_refs(format: DocumentFormat, bytes: &[u8], limit: u64) -> Vec<String> {
    let mut refs: Vec<String> = import_rich_text(format, bytes)
        .map(|model| media_refs(&model))
        .unwrap_or_default()
        .into_iter()
        .filter(|reference| ContentHash::try_new(reference).is_ok())
        .collect();
    refs.sort();
    refs.dedup();
    let packaged = match format {
        DocumentFormat::Docx => saved_uploads(bytes, &refs, limit),
        DocumentFormat::Odt => refs
            .iter()
            .filter(|reference| media_part(bytes, "Pictures/", reference, limit).is_some())
            .cloned()
            .collect(),
        _ => Vec::new(),
    };
    refs.retain(|reference| !packaged.contains(reference));
    refs
}
