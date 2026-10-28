//! The editable body of a live edit session.
//!
//! A session's engine document holds the editable body of the document as
//! [`ModelView::Body`](crate::documents::engine::ModelView): the inner markup
//! of `<body>` for HTML (the whole document when it has no `<body>` element)
//! and the full text for Markdown. [`materialise`] turns the live body back
//! into document bytes, keeping the HTML around `<body>` from the revision
//! the session is based on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::documents::DocumentFormat;

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

#[must_use]
pub fn materialise(format: DocumentFormat, base: &str, body: &str) -> String {
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
