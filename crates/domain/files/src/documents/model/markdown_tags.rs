//! The Markdown constructs that map one to one onto the model: heading
//! levels, GitHub alert kinds as callout tones, and the inline HTML tags
//! read and written as marks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pulldown_cmark::{BlockQuoteKind, HeadingLevel};

use super::layout::CalloutTone;

pub(super) const INLINE_TAGS: [(&str, &str); 3] = [
    ("u", "underline"),
    ("sup", "superscript"),
    ("sub", "subscript"),
];

pub(super) const fn tone(kind: BlockQuoteKind) -> CalloutTone {
    match kind {
        BlockQuoteKind::Note => CalloutTone::Note,
        BlockQuoteKind::Tip => CalloutTone::Success,
        BlockQuoteKind::Important => CalloutTone::Info,
        BlockQuoteKind::Warning => CalloutTone::Warning,
        BlockQuoteKind::Caution => CalloutTone::Danger,
    }
}

pub(super) const fn heading(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
