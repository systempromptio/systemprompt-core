//! Canonical extension ↔ MIME mapping for the platform.
//!
//! This module is the single source of truth. Every surface that needs to name
//! the type of a file — the HTTP static-asset and plugin-file handlers, the
//! file-ingestion job, the CLI uploader, and the bridge GUI's embedded asset
//! server — resolves through here rather than carrying its own table.
//!
//! # Essence versus served form
//!
//! Two forms of the same type are needed and they are not interchangeable.
//!
//! [`from_path`] and [`from_extension`] return the *essence* — `type/subtype`
//! with no parameters. This is the form to store in a database column, hand to
//! an upload validator, or compare against an allowlist; a `charset` parameter
//! on either side of such a comparison makes it fail.
//!
//! [`http_content_type`] returns the *served* form, which appends
//! `charset=utf-8` to the `text/*` types. This is the form for a
//! `Content-Type` response header and nothing else. The parameter is confined
//! to `text/*` deliberately: RFC 8259 defines no `charset` for
//! `application/json`, and YAML and TOML are likewise UTF-8 by definition.
//!
//! [`extension_for`] inverts the mapping, tolerating parameters and known
//! aliases on its input.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

const OCTET_STREAM: &str = "application/octet-stream";

const TABLE: &[(&[&str], &str, &str)] = &[
    (&["png"], "image/png", "image/png"),
    (&["jpg", "jpeg"], "image/jpeg", "image/jpeg"),
    (&["webp"], "image/webp", "image/webp"),
    (&["gif"], "image/gif", "image/gif"),
    (&["avif"], "image/avif", "image/avif"),
    (&["bmp"], "image/bmp", "image/bmp"),
    (&["tiff", "tif"], "image/tiff", "image/tiff"),
    (&["ico"], "image/x-icon", "image/x-icon"),
    (&["svg"], "image/svg+xml", "image/svg+xml"),
    (&["mp4"], "video/mp4", "video/mp4"),
    (&["webm"], "video/webm", "video/webm"),
    (&["mov"], "video/quicktime", "video/quicktime"),
    (&["avi"], "video/x-msvideo", "video/x-msvideo"),
    (&["mkv"], "video/x-matroska", "video/x-matroska"),
    (&["ogv"], "video/ogg", "video/ogg"),
    (&["mp3"], "audio/mpeg", "audio/mpeg"),
    (&["weba"], "audio/webm", "audio/webm"),
    (&["wav"], "audio/wav", "audio/wav"),
    (&["ogg"], "audio/ogg", "audio/ogg"),
    (&["aac"], "audio/aac", "audio/aac"),
    (&["flac"], "audio/flac", "audio/flac"),
    (&["m4a"], "audio/mp4", "audio/mp4"),
    (&["woff"], "font/woff", "font/woff"),
    (&["woff2"], "font/woff2", "font/woff2"),
    (&["ttf"], "font/ttf", "font/ttf"),
    (&["otf"], "font/otf", "font/otf"),
    (&["html", "htm"], "text/html", "text/html; charset=utf-8"),
    (&["css"], "text/css", "text/css; charset=utf-8"),
    (
        &["js", "mjs"],
        "text/javascript",
        "text/javascript; charset=utf-8",
    ),
    (&["json", "map"], "application/json", "application/json"),
    (&["txt"], "text/plain", "text/plain; charset=utf-8"),
    (&["csv"], "text/csv", "text/csv; charset=utf-8"),
    (&["md"], "text/markdown", "text/markdown; charset=utf-8"),
    (&["ftl"], "text/plain", "text/plain; charset=utf-8"),
    (&["xml"], "application/xml", "application/xml"),
    (&["yaml", "yml"], "application/yaml", "application/yaml"),
    (&["toml"], "application/toml", "application/toml"),
    (&["sh"], "application/x-sh", "application/x-sh"),
    (&["wasm"], "application/wasm", "application/wasm"),
    (&["pdf"], "application/pdf", "application/pdf"),
    (&["rtf"], "application/rtf", "application/rtf"),
    (&["doc"], "application/msword", "application/msword"),
    (
        &["docx"],
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ),
    (
        &["xls"],
        "application/vnd.ms-excel",
        "application/vnd.ms-excel",
    ),
    (
        &["xlsx"],
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ),
    (
        &["ppt"],
        "application/vnd.ms-powerpoint",
        "application/vnd.ms-powerpoint",
    ),
    (
        &["pptx"],
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    ),
    // Why: without these rows the static handler labels an archive
    // `text/plain; charset=utf-8` and a browser renders mojibake instead of saving.
    (&["gz", "tgz"], "application/gzip", "application/gzip"),
    (&["tar"], "application/x-tar", "application/x-tar"),
    (&["zip"], "application/zip", "application/zip"),
    (&["bz2"], "application/x-bzip2", "application/x-bzip2"),
    (&["xz"], "application/x-xz", "application/x-xz"),
    (&["zst"], "application/zstd", "application/zstd"),
    (&["exe", "msi"], OCTET_STREAM, OCTET_STREAM),
    (
        &["dmg"],
        "application/x-apple-diskimage",
        "application/x-apple-diskimage",
    ),
    (
        &["deb"],
        "application/vnd.debian.binary-package",
        "application/vnd.debian.binary-package",
    ),
    (&["rpm"], "application/x-rpm", "application/x-rpm"),
    (&["appimage"], OCTET_STREAM, OCTET_STREAM),
    // Why: checksum and signature sidecars are meant to be read in the browser,
    // not downloaded, so they stay text/plain rather than octet-stream.
    (
        &["sha256", "sha512", "asc"],
        "text/plain",
        "text/plain; charset=utf-8",
    ),
    (&["sig"], OCTET_STREAM, OCTET_STREAM),
];

const ALIASES: &[(&str, &str)] = &[
    ("image/vnd.microsoft.icon", "image/x-icon"),
    ("text/xml", "application/xml"),
    ("application/javascript", "text/javascript"),
    ("text/yaml", "application/yaml"),
    ("audio/mp3", "audio/mpeg"),
    ("audio/wave", "audio/wav"),
    ("audio/x-wav", "audio/wav"),
    ("audio/x-m4a", "audio/mp4"),
];

fn lookup(ext: &str) -> Option<&'static (&'static [&'static str], &'static str, &'static str)> {
    let lower = ext.to_ascii_lowercase();
    TABLE
        .iter()
        .find(|(exts, _, _)| exts.iter().any(|e| *e == lower))
}

fn extension_of(path: &Path) -> Option<&str> {
    path.extension().and_then(std::ffi::OsStr::to_str)
}

/// Matched case-insensitively.
pub fn from_extension(ext: &str) -> Option<&'static str> {
    lookup(ext).map(|(_, essence, _)| *essence)
}

pub fn from_path(path: &Path) -> &'static str {
    extension_of(path)
        .and_then(from_extension)
        .unwrap_or(OCTET_STREAM)
}

pub fn http_content_type(path: &Path) -> &'static str {
    http_content_type_opt(path).unwrap_or(OCTET_STREAM)
}

pub fn http_content_type_opt(path: &Path) -> Option<&'static str> {
    extension_of(path)
        .and_then(lookup)
        .map(|(_, _, served)| *served)
}

pub fn extension_for(mime: &str) -> Option<&'static str> {
    let essence = essence_of(mime);
    let resolved = ALIASES
        .iter()
        .find(|(alias, _)| *alias == essence)
        .map_or(essence.as_str(), |(_, canonical)| *canonical);

    // Why: several concrete rows (exe/msi, appimage, sig) share octet-stream as
    // their essence; taking the first would name an unknown upload `.exe`.
    if resolved == OCTET_STREAM {
        return Some("bin");
    }

    TABLE
        .iter()
        .find(|(_, e, _)| *e == resolved)
        .and_then(|(exts, _, _)| exts.first().copied())
}

/// Comparing a client-supplied `Content-Type` against an allowlist or a
/// blocklist without this admits `text/javascript; charset=utf-8` past a
/// blocklist holding `text/javascript`.
pub fn essence_of(mime: &str) -> String {
    mime.split(';')
        .next()
        .unwrap_or(mime)
        .trim()
        .to_ascii_lowercase()
}
