//! SVG uploads are refused, not sanitised, when they carry active or
//! external content: scripts, event-handler attributes, `javascript:` or
//! `data:` URLs, `foreignObject`, embedded documents, entity declarations
//! or a reference to a remote resource. A plain vector drawing passes
//! unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::uploads::error::{RefusalReason, UploadError, UploadResult};

const FORBIDDEN: [&str; 12] = [
    "<script",
    "javascript:",
    "vbscript:",
    "\"data:",
    "'data:",
    "(data:",
    "<foreignobject",
    "<iframe",
    "<embed",
    "<object",
    "<!entity",
    "<?xml-stylesheet",
];

const REMOTE: [&str; 4] = ["href=\"http", "href='http", "url(http", "@import"];

pub(crate) fn check(bytes: &[u8]) -> UploadResult<()> {
    let text = std::str::from_utf8(bytes).map_err(|_not_utf8| {
        UploadError::refused(RefusalReason::UnsafeSvg, "the SVG is not UTF-8")
    })?;
    let lowered: String = text
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let found = FORBIDDEN
        .iter()
        .chain(REMOTE.iter())
        .find(|needle| lowered.contains(*needle))
        .copied()
        .or_else(|| has_event_handler(&lowered).then_some("on* event handler"));
    found.map_or_else(
        || Ok(()),
        |what| {
            Err(UploadError::refused(
                RefusalReason::UnsafeSvg,
                format!("the SVG carries active or external content ({what})"),
            ))
        },
    )
}

fn has_event_handler(lowered: &str) -> bool {
    lowered.match_indices(" on").any(|(at, _)| {
        let rest = &lowered[at + 3..];
        let name_len = rest.bytes().take_while(u8::is_ascii_alphabetic).count();
        name_len > 0 && rest[name_len..].trim_start().starts_with('=')
    })
}
