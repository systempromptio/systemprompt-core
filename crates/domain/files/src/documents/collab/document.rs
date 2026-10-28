//! A document's bytes as the live session's model, and back.
//!
//! A new session seeds its engine document with [`import`]: the document as
//! a `RichText` model, read from the inner markup of `<body>` for HTML, the
//! full text for Markdown, and the body of `word/document.xml` or
//! `content.xml` for `docx` and `odt`. [`materialise`] turns the live model
//! back into document bytes on top of the revision the session is based on,
//! keeping everything the model does not own. A session opened before the
//! model existed still holds a flat `ModelView::Body` and materialises the
//! way it always did. [`unchanged_from`] compares a model with its base
//! after both are materialised, since writing normalises markup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::documents::DocumentFormat;
use systemprompt_provider_contracts::documents::CodecError;

use super::model::{body_of, splice_body};
use crate::documents::engine::ModelView;
use crate::documents::model::{RichText, html, markdown};

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("the document is not UTF-8 text")]
    NotText(#[from] std::str::Utf8Error),
    #[error("the document could not be read as its format")]
    Codec(#[from] CodecError),
    #[error("live sessions do not co-edit this format")]
    Unsupported,
}

pub fn import(format: DocumentFormat, document: &[u8]) -> Result<ModelView, ModelError> {
    Ok(ModelView::RichText(import_rich_text(format, document)?))
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

pub fn materialise(
    format: DocumentFormat,
    base: &[u8],
    model: &ModelView,
) -> Result<Vec<u8>, ModelError> {
    match model {
        ModelView::RichText(rich) => match format {
            DocumentFormat::Markdown => Ok(markdown::materialise(rich).into_bytes()),
            DocumentFormat::Html => {
                let base = std::str::from_utf8(base)?;
                Ok(splice_body(format, base, &html::materialise(rich)).into_bytes())
            },
            _ => office_materialise(format, base, rich),
        },
        ModelView::Body(body) => {
            Ok(splice_body(format, std::str::from_utf8(base)?, body).into_bytes())
        },
    }
}

pub fn unchanged_from(
    format: DocumentFormat,
    base: &[u8],
    model: &ModelView,
) -> Result<bool, ModelError> {
    let next = materialise(format, base, model)?;
    Ok(match model {
        ModelView::RichText(_) => next == materialise(format, base, &import(format, base)?)?,
        ModelView::Body(_) => next == base,
    })
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
) -> Result<Vec<u8>, ModelError> {
    use crate::documents::codecs::{docx, odf};
    match format {
        DocumentFormat::Docx => Ok(docx::materialise_rich(base, model)?),
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
) -> Result<Vec<u8>, ModelError> {
    Err(ModelError::Unsupported)
}
