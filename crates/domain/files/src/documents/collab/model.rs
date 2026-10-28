//! The editable model of a live edit session.
//!
//! A new session seeds its engine document with [`import`]: the document as
//! a [`RichText`] model, read from the inner markup of `<body>` for HTML
//! (the whole document when it has no `<body>` element) and from the full
//! text for Markdown. [`materialise`] turns the live model back into
//! document text, keeping the HTML around `<body>` from the revision the
//! session is based on. A session opened before the model existed still
//! holds a flat [`ModelView::Body`] and materialises the same way it always
//! did.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::documents::DocumentFormat;

use crate::documents::engine::ModelView;
use crate::documents::model::{RichText, html, markdown};

#[must_use]
pub const fn is_collab_format(format: DocumentFormat) -> bool {
    matches!(format, DocumentFormat::Html | DocumentFormat::Markdown)
}

#[must_use]
pub fn body_of(format: DocumentFormat, document: &str) -> String {
    match (format, body_span(document)) {
        (DocumentFormat::Html, Some((start, end))) => document[start..end].to_owned(),
        _ => document.to_owned(),
    }
}

/// The document's editable content as the model a new session seeds.
#[must_use]
pub fn import(format: DocumentFormat, document: &str) -> ModelView {
    ModelView::RichText(import_rich_text(format, document))
}

#[must_use]
pub fn import_rich_text(format: DocumentFormat, document: &str) -> RichText {
    match format {
        DocumentFormat::Markdown => markdown::import(document),
        _ => html::import(&body_of(format, document)),
    }
}

/// The document text `model` stands for, on top of `base`.
#[must_use]
pub fn materialise(format: DocumentFormat, base: &str, model: &ModelView) -> String {
    match model {
        ModelView::RichText(rich) => {
            let body = match format {
                DocumentFormat::Markdown => markdown::materialise(rich),
                _ => html::materialise(rich),
            };
            splice_body(format, base, &body)
        },
        ModelView::Body(body) => splice_body(format, base, body),
    }
}

/// Whether `model` says nothing `base` does not: importing `base` and
/// materialising both gives the same text. Materialising normalises markup,
/// so comparing against `base` itself would commit an unedited session.
#[must_use]
pub fn unchanged_from(format: DocumentFormat, base: &str, model: &ModelView) -> bool {
    match model {
        ModelView::RichText(_) => {
            materialise(format, base, model) == materialise(format, base, &import(format, base))
        },
        ModelView::Body(_) => materialise(format, base, model) == base,
    }
}

#[must_use]
pub fn splice_body(format: DocumentFormat, base: &str, body: &str) -> String {
    match (format, body_span(base)) {
        (DocumentFormat::Html, Some((start, end))) => {
            let mut out = String::with_capacity(base.len() - (end - start) + body.len());
            out.push_str(&base[..start]);
            out.push_str(body);
            out.push_str(&base[end..]);
            out
        },
        _ => body.to_owned(),
    }
}

fn body_span(html: &str) -> Option<(usize, usize)> {
    let lower = html.to_ascii_lowercase();
    let open = find_tag(&lower, "<body")?;
    let start = open + lower[open..].find('>')? + 1;
    let end = lower.rfind("</body")?;
    (end >= start).then_some((start, end))
}

fn find_tag(lower: &str, tag: &str) -> Option<usize> {
    lower.match_indices(tag).map(|(at, _)| at).find(|at| {
        lower[at + tag.len()..]
            .chars()
            .next()
            .is_some_and(|c| c == '>' || c.is_ascii_whitespace() || c == '/')
    })
}
