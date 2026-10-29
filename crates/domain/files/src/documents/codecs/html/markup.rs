//! Markup the HTML codec writes for inserted Markdown: each run of lines
//! rendered and sanitised, and each pending-intent line written as a
//! placeholder paragraph the agent fills later.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::pending::intent_of_line;
use super::super::text::escape_html;
use super::tree::PENDING_ATTR;

pub(super) fn markdown_chunk(markdown: &str) -> String {
    let mut rendered = String::new();
    pulldown_cmark::html::push_html(
        &mut rendered,
        pulldown_cmark::Parser::new_ext(
            markdown,
            pulldown_cmark::Options::ENABLE_TABLES | pulldown_cmark::Options::ENABLE_STRIKETHROUGH,
        ),
    );
    ammonia::clean(&rendered)
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
