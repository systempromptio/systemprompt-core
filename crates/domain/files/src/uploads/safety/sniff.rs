//! The declared MIME types an upload may carry, and the magic-byte sniff
//! that checks the first bytes really are that kind.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Zip,
    Pdf,
    Png,
    Jpeg,
    Gif,
    Webp,
    Svg,
    Text,
}

const OFFICE_PREFIXES: [&str; 2] = [
    "application/vnd.openxmlformats-officedocument.",
    "application/vnd.oasis.opendocument.",
];

/// The kind a declared MIME type must sniff as, or `None` when uploads of
/// that type are not accepted at all.
pub(crate) fn declared_kind(mime: &str) -> Option<Kind> {
    let mime = mime.split(';').next().unwrap_or_default().trim();
    if OFFICE_PREFIXES
        .iter()
        .any(|prefix| mime.starts_with(prefix))
    {
        return Some(Kind::Zip);
    }
    match mime {
        "application/pdf" => Some(Kind::Pdf),
        "image/png" => Some(Kind::Png),
        "image/jpeg" => Some(Kind::Jpeg),
        "image/gif" => Some(Kind::Gif),
        "image/webp" => Some(Kind::Webp),
        "image/svg+xml" => Some(Kind::Svg),
        "text/plain" | "text/markdown" | "text/html" | "text/csv" => Some(Kind::Text),
        _ => None,
    }
}

/// Whether the first bytes are what the declared kind must be. SVG is text
/// that opens an `<svg` element; any other text is only text, so an HTML
/// page with an inline drawing still sniffs as the text it was declared.
pub(crate) fn matches(expected: Kind, head: &[u8]) -> bool {
    match (expected, sniff(head)) {
        (Kind::Svg, Some(Kind::Text)) => {
            utf8_prefix(head).is_some_and(|text| text.to_ascii_lowercase().contains("<svg"))
        },
        (expected, found) => found == Some(expected),
    }
}

/// The kind the first bytes of an upload are, judged from magic bytes; text
/// is valid UTF-8 without NUL bytes.
pub(crate) fn sniff(head: &[u8]) -> Option<Kind> {
    let magic: [(&[u8], Kind); 6] = [
        (b"PK\x03\x04", Kind::Zip),
        (b"%PDF-", Kind::Pdf),
        (b"\x89PNG\r\n\x1a\n", Kind::Png),
        (b"\xff\xd8\xff", Kind::Jpeg),
        (b"GIF87a", Kind::Gif),
        (b"GIF89a", Kind::Gif),
    ];
    if let Some((_, kind)) = magic.iter().find(|(prefix, _)| head.starts_with(prefix)) {
        return Some(*kind);
    }
    if head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        return Some(Kind::Webp);
    }
    let text = utf8_prefix(head)?;
    (!text.contains('\0')).then_some(Kind::Text)
}

fn utf8_prefix(head: &[u8]) -> Option<&str> {
    match std::str::from_utf8(head) {
        Ok(text) => Some(text),
        Err(error) if error.error_len().is_none() => {
            std::str::from_utf8(&head[..error.valid_up_to()]).ok()
        },
        Err(_) => None,
    }
}

/// The MIME type recorded for a sniffed kind.
pub(crate) const fn detected_mime(kind: Kind, declared: &str) -> &str {
    match kind {
        Kind::Zip | Kind::Text => declared,
        Kind::Pdf => "application/pdf",
        Kind::Png => "image/png",
        Kind::Jpeg => "image/jpeg",
        Kind::Gif => "image/gif",
        Kind::Webp => "image/webp",
        Kind::Svg => "image/svg+xml",
    }
}
