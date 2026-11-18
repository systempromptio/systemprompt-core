//! Markup the HTML codec writes for inserted Markdown: each run of lines
//! rendered and sanitised, and each pending-intent line written as a
//! placeholder paragraph the agent fills later.
//!
//! One sanitiser serves the whole codec. It keeps what the codec itself
//! writes (the pending-block attribute, a link's `rel`) and adds nothing, so
//! markup the codec wrote passes it unchanged. A body edit whose result the
//! sanitiser would alter is refused rather than silently stripped.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::LazyLock;

use scraper::Html;
use systemprompt_provider_contracts::documents::CodecError;

use super::super::pending::intent_of_line;
use super::super::text::escape_html;
use super::tree::PENDING_ATTR;

const STRIPPED: &str = "the edited body holds markup an edit op would strip (a script, style, \
                        class, event handler, comment or other attribute the sanitiser drops); \
                        edit ops never write it. Replace the whole file instead: upload the page \
                        with documents_upload and call documents_edit {document_id, \
                        content_blob_ref}";

static SANITISER: LazyLock<ammonia::Builder<'static>> = LazyLock::new(|| {
    let mut builder = ammonia::Builder::default();
    builder
        .link_rel(None)
        .add_generic_attributes(&[PENDING_ATTR])
        .add_tag_attributes("a", &["rel"]);
    builder
});

fn sanitise(markup: &str) -> String {
    SANITISER.clean(markup).to_string()
}

fn normalised(markup: &str) -> String {
    Html::parse_fragment(markup).root_element().inner_html()
}

pub(super) fn unaltered_body(markup: &str) -> Result<String, CodecError> {
    let cleaned = sanitise(markup);
    if normalised(&cleaned) == normalised(markup) {
        Ok(cleaned)
    } else {
        Err(CodecError::Refused {
            detail: STRIPPED.to_owned(),
        })
    }
}

pub(super) fn markdown_chunk(markdown: &str) -> String {
    let mut rendered = String::new();
    pulldown_cmark::html::push_html(
        &mut rendered,
        pulldown_cmark::Parser::new_ext(
            markdown,
            pulldown_cmark::Options::ENABLE_TABLES | pulldown_cmark::Options::ENABLE_STRIKETHROUGH,
        ),
    );
    sanitise(&rendered)
}

pub(super) fn pending_markup(intent: &str) -> String {
    let intent = escape_html(intent);
    format!("<p {PENDING_ATTR}=\"{intent}\"><em>{intent}</em></p>")
}

pub(super) fn markdown_markup(markdown: &str) -> String {
    let mut out = String::new();
    let mut chunk = String::new();
    for line in markdown.lines() {
        if let Some(intent) = intent_of_line(line) {
            out.push_str(&markdown_chunk(&chunk));
            chunk.clear();
            out.push_str(&pending_markup(intent));
        } else {
            chunk.push_str(line);
            chunk.push('\n');
        }
    }
    out.push_str(&markdown_chunk(&chunk));
    out
}
