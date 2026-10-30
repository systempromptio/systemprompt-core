//! Which formats a live session co-edits, and the HTML `<body>` splicing
//! its text formats share.
//!
//! HTML and Markdown are always co-edited; with the `office` feature so are
//! `docx` and `odt`, and `xlsx` workbooks held as a Loro sheet (IronCalc
//! cannot read `ods`).
//! The `<body>` helpers live with the model, in `model::html_body`, and are
//! re-exported here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::documents::DocumentFormat;

pub use crate::documents::model::html_body::{body_of, splice_body};

#[must_use]
pub const fn is_collab_format(format: DocumentFormat) -> bool {
    matches!(format, DocumentFormat::Html | DocumentFormat::Markdown) || is_office_text(format)
}

#[cfg(feature = "office")]
const fn is_office_text(format: DocumentFormat) -> bool {
    matches!(
        format,
        DocumentFormat::Docx
            | DocumentFormat::Odt
            | DocumentFormat::Xlsx
            | DocumentFormat::Pptx
    )
}

#[cfg(not(feature = "office"))]
const fn is_office_text(_format: DocumentFormat) -> bool {
    false
}
