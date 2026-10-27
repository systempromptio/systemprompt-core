//! Pending placeholders: a block that stands for content still to be
//! written, carrying the intent of that content.
//!
//! Each codec stores a placeholder in its own native form and projects it
//! back as kind `pending` with the intent as its text; `replace_text` on it
//! writes ordinary content in its place. The line formats store one line:
//! `[pending: intent]` in text, and `<!-- pending: intent --> *intent*` in
//! Markdown, where the comment carries the marker and the italics show the
//! intent to a reader.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::DocumentFormat;

const TEXT_OPEN: &str = "[pending: ";
const TEXT_CLOSE: &str = "]";
const MARKDOWN_OPEN: &str = "<!-- pending: ";
const MARKDOWN_CLOSE: &str = " -->";

pub(crate) fn text_marker(intent: &str) -> String {
    format!("{TEXT_OPEN}{}{TEXT_CLOSE}", one_line(intent))
}

pub(crate) fn markdown_marker(intent: &str) -> String {
    let intent = one_line(intent);
    format!("{MARKDOWN_OPEN}{intent}{MARKDOWN_CLOSE} *{intent}*")
}

pub(crate) fn line_marker(format: DocumentFormat, intent: &str) -> String {
    match format {
        DocumentFormat::Markdown => markdown_marker(intent),
        _ => text_marker(intent),
    }
}

pub(crate) fn intent_of_line(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix(MARKDOWN_OPEN) {
        return rest.split_once(MARKDOWN_CLOSE).map(|(intent, _)| intent);
    }
    trimmed
        .strip_prefix(TEXT_OPEN)
        .and_then(|rest| rest.strip_suffix(TEXT_CLOSE))
}

fn one_line(intent: &str) -> String {
    intent.split_whitespace().collect::<Vec<_>>().join(" ")
}
