//! The pass-through codec of [`DocumentFormat::File`]: bytes stored as they
//! were uploaded.
//!
//! A file projects to no nodes and has no plain text, so it is listed,
//! downloaded and shared like any document but never edited by op: every op
//! is refused, and a new version is a byte write of the whole file. Its
//! preview shows a PNG, JPEG, GIF or WebP image inline (as a `data:` URL,
//! which the preview's CSP allows) up to [`PREVIEW_IMAGE_BYTES`]; the HTTP
//! preview route serves a PDF itself, and anything else previews as a
//! short note.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use base64::Engine;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, Projection, TemplateParams, content_hash,
};
use systemprompt_provider_contracts::register_document_codec;

pub const PREVIEW_IMAGE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default)]
pub struct FileCodec;

const FORMATS: &[DocumentFormat] = &[DocumentFormat::File];

const NO_PREVIEW: &str = "<p>This file has no preview. Download it to open it.</p>";

fn factory() -> Arc<dyn DocumentCodec> {
    Arc::new(FileCodec)
}

register_document_codec!(factory);

/// The image type of `bytes` from its magic bytes, if it is one the
/// preview shows.
#[must_use]
pub fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

#[must_use]
pub fn is_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-")
}

impl DocumentCodec for FileCodec {
    fn insert_kinds(&self, _format: DocumentFormat) -> &'static [&'static str] {
        &[]
    }

    fn formats(&self) -> &'static [DocumentFormat] {
        FORMATS
    }

    fn project(&self, bytes: &[u8]) -> Result<Projection, CodecError> {
        Ok(Projection {
            format: DocumentFormat::File,
            root: Vec::new(),
            content_sha256: content_hash(bytes),
        })
    }

    fn apply(
        &self,
        bytes: &[u8],
        _base: &Projection,
        ops: &[EditOp],
    ) -> Result<Vec<u8>, CodecError> {
        match ops.first() {
            None => Ok(bytes.to_vec()),
            Some(op) => Err(CodecError::UnsupportedOp {
                op: op.name(),
                format: DocumentFormat::File,
            }),
        }
    }

    fn preview_html(&self, bytes: &[u8]) -> Result<String, CodecError> {
        let Some(mime) = image_mime(bytes).filter(|_| bytes.len() <= PREVIEW_IMAGE_BYTES) else {
            return Ok(if is_pdf(bytes) {
                "<p>PDF document. Download it, or open its preview, to read it.</p>".to_owned()
            } else {
                NO_PREVIEW.to_owned()
            });
        };
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        Ok(format!(
            "<img alt=\"\" style=\"max-width:100%;height:auto\" src=\"data:{mime};base64,{encoded}\">"
        ))
    }

    fn plain_text(&self, _bytes: &[u8]) -> Result<String, CodecError> {
        Ok(String::new())
    }

    fn instantiate(
        &self,
        template: &[u8],
        _params: &TemplateParams,
    ) -> Result<Vec<u8>, CodecError> {
        Ok(template.to_vec())
    }
}
