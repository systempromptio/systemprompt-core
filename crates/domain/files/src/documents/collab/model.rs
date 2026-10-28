//! The Loro document model of a live edit session.
//!
//! A session's `LoroDoc` holds one [`LoroText`] container named
//! [`BODY_CONTAINER`] whose value is the editable body of the document: the
//! inner markup of `<body>` for HTML (the whole document when it has no
//! `<body>` element) and the full text for Markdown. A browser editor mirrors
//! the same container with `loro-crdt`.
//!
//! [`seed`] writes the initial body with peer id [`SEED_PEER`] and timestamps
//! off, then moves to a random peer id, so every replica that seeds from the
//! same revision produces the same operations and can import any client's
//! updates without a shared snapshot. [`materialise`] turns the live body back
//! into document bytes, keeping the HTML around `<body>` from the revision the
//! session is based on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{LoroDoc, LoroError, LoroText, PeerID};
use systemprompt_models::documents::DocumentFormat;

pub const BODY_CONTAINER: &str = "body";
pub const SEED_PEER: PeerID = 0;

#[must_use]
pub const fn is_collab_format(format: DocumentFormat) -> bool {
    matches!(format, DocumentFormat::Html | DocumentFormat::Markdown)
}

#[must_use]
pub fn body_text(doc: &LoroDoc) -> String {
    doc.get_text(BODY_CONTAINER).to_string()
}

pub fn seed(body: &str) -> Result<LoroDoc, LoroError> {
    let doc = LoroDoc::new();
    doc.set_peer_id(SEED_PEER)?;
    let text: LoroText = doc.get_text(BODY_CONTAINER);
    text.insert(0, body)?;
    doc.commit();
    doc.set_peer_id(replica_peer())?;
    Ok(doc)
}

#[must_use]
pub fn replica_peer() -> PeerID {
    uuid::Uuid::new_v4().as_u64_pair().0.max(1)
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
