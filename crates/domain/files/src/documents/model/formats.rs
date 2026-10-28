//! A document's bytes as a `RichText` model, and back, for every format the
//! model covers.
//!
//! Markdown imports from its full text and materialises as a whole; HTML
//! imports the inner markup of `<body>` and writes it back inside the
//! original document; with the `office` feature `docx` and `odt` import their
//! bodies and splice the model back, keeping every block it did not change.
//! The live session and the rich-text edit ops both go through here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::documents::DocumentFormat;
use systemprompt_provider_contracts::documents::CodecError;

use super::html_body::{body_of, splice_body};
use super::{Media, RichText, html, markdown};

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("the document is not UTF-8 text")]
    NotText(#[from] std::str::Utf8Error),
    #[error("the document could not be read as its format")]
    Codec(#[from] CodecError),
    #[error("live sessions do not co-edit this format")]
    Unsupported,
}

pub fn import_rich_text(format: DocumentFormat, document: &[u8]) -> Result<RichText, ModelError> {
    match format {
        DocumentFormat::Markdown => Ok(markdown::import(std::str::from_utf8(document)?)),
        DocumentFormat::Html => Ok(html::import(&body_of(
            format,
            std::str::from_utf8(document)?,
        ))),
        _ => office_import(format, document),
    }
}

pub fn materialise_rich_text(
    format: DocumentFormat,
    base: &[u8],
    model: &RichText,
) -> Result<Vec<u8>, ModelError> {
    materialise_rich_text_with(format, base, model, &Media::new())
}

/// The live-session materialise: like [`materialise_rich_text_with`], but a
/// Markdown model is first degraded to what Markdown can hold, so a live
/// checkpoint never fails on it; the warnings name what was left out.
pub fn materialise_rich_text_live(
    format: DocumentFormat,
    base: &[u8],
    model: &RichText,
    media: &Media,
) -> Result<(Vec<u8>, Vec<String>), ModelError> {
    if format != DocumentFormat::Markdown {
        return Ok((
            materialise_rich_text_with(format, base, model, media)?,
            Vec::new(),
        ));
    }
    let (degraded, warnings) = markdown::degrade(model);
    Ok((markdown::materialise(&degraded)?.into_bytes(), warnings))
}

/// [`materialise_rich_text`] with the bytes of the images the model adds;
/// see [`super::media_refs`]. Only `docx` embeds images, so only `docx`
/// reads `media`.
pub fn materialise_rich_text_with(
    format: DocumentFormat,
    base: &[u8],
    model: &RichText,
    media: &Media,
) -> Result<Vec<u8>, ModelError> {
    match format {
        DocumentFormat::Markdown => Ok(markdown::materialise(model)?.into_bytes()),
        DocumentFormat::Html => {
            let base = std::str::from_utf8(base)?;
            Ok(splice_body(format, base, &html::materialise(model)).into_bytes())
        },
        _ => office_materialise(format, base, model, media),
    }
}

#[cfg(feature = "office")]
fn office_import(format: DocumentFormat, document: &[u8]) -> Result<RichText, ModelError> {
    use crate::documents::codecs::{docx, odf};
    match format {
        DocumentFormat::Docx => Ok(docx::import_rich(document)?),
        DocumentFormat::Odt => Ok(odf::import_odt_rich(document)?),
        _ => Err(ModelError::Unsupported),
    }
}

#[cfg(feature = "office")]
fn office_materialise(
    format: DocumentFormat,
    base: &[u8],
    model: &RichText,
    media: &Media,
) -> Result<Vec<u8>, ModelError> {
    use crate::documents::codecs::{docx, odf};
    match format {
        DocumentFormat::Docx => Ok(docx::materialise_rich_with(base, model, media)?),
        DocumentFormat::Odt => Ok(odf::materialise_odt_rich(base, model)?),
        _ => Err(ModelError::Unsupported),
    }
}

#[cfg(not(feature = "office"))]
const fn office_import(_format: DocumentFormat, _document: &[u8]) -> Result<RichText, ModelError> {
    Err(ModelError::Unsupported)
}

#[cfg(not(feature = "office"))]
const fn office_materialise(
    _format: DocumentFormat,
    _base: &[u8],
    _model: &RichText,
    _media: &Media,
) -> Result<Vec<u8>, ModelError> {
    Err(ModelError::Unsupported)
}
