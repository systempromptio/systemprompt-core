//! A document's bytes as the live session's model, and back.
//!
//! A new session seeds its engine document with [`import`]: the document as
//! a `RichText` model, through `model::formats` (the inner markup of
//! `<body>` for HTML, the full text for Markdown, and the body of
//! `word/document.xml` or `content.xml` for `docx` and `odt`).
//! [`materialise`] turns the live model back into document bytes on top of
//! the revision the session is based on,
//! keeping everything the model does not own. A session opened before the
//! model existed still holds a flat `ModelView::Body` and materialises the
//! way it always did. [`unchanged_from`] compares a model with its base
//! after both are materialised, since writing normalises markup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::documents::DocumentFormat;

use super::model::splice_body;
use crate::documents::engine::ModelView;
use crate::documents::model::formats::materialise_rich_text;

pub use crate::documents::model::formats::{ModelError, import_rich_text};

pub fn import(format: DocumentFormat, document: &[u8]) -> Result<ModelView, ModelError> {
    Ok(ModelView::RichText(import_rich_text(format, document)?))
}

pub fn materialise(
    format: DocumentFormat,
    base: &[u8],
    model: &ModelView,
) -> Result<Vec<u8>, ModelError> {
    match model {
        ModelView::RichText(rich) => materialise_rich_text(format, base, rich),
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
