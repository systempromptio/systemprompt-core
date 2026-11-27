//! Which names the Drive folder holds: portable file names, case collisions,
//! conflict copies and ignored files.
//!
//! A path from the instance is placed only when every desktop filesystem can
//! hold it under that exact name, and no other synced file folds to the same
//! key (NFC, lowercase, NFC again), since NTFS and APFS compare names without
//! case and APFS without normalisation form. Editor lock files, partial
//! downloads, temporary files and every hidden name (the index, `.trash`,
//! staged writes) are never synced.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use unicode_normalization::UnicodeNormalization as _;

const RESERVED: [&str; 6] = ["con", "prn", "aux", "nul", "conin$", "conout$"];
const DEVICE_DIGITS: [&str; 12] = [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "\u{b9}", "\u{b2}", "\u{b3}",
];
const DEVICE_PREFIXES: [&str; 2] = ["com", "lpt"];
const REFUSED: [char; 9] = ['<', '>', '"', '|', '?', '*', ':', '\\', '/'];
const IGNORED_EXTENSIONS: [&str; 6] = ["tmp", "crdownload", "part", "partial", "swp", "download"];
const IGNORED_NAMES: [&str; 2] = ["desktop.ini", "thumbs.db"];

/// Why a path names no file this device can place.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathRefusal {
    #[error("it is not a plain relative path")]
    NotRelative,
    #[error("it is a reserved device name on Windows")]
    ReservedName,
    #[error("it ends in a dot or a space")]
    TrailingDotOrSpace,
    #[error("it holds a character Windows refuses in a name")]
    RefusedCharacter,
    #[error("it is a name the Drive folder keeps for itself")]
    Hidden,
    #[error("it lies under a link")]
    Link,
    #[error("a file this device does not sync already holds the name")]
    Occupied,
    #[error("it differs only in case or Unicode form from {existing}")]
    CaseCollision { existing: String },
}

pub(crate) fn portable_segment(segment: &str) -> Result<(), PathRefusal> {
    if segment.is_empty() || segment == "." || segment == ".." {
        return Err(PathRefusal::NotRelative);
    }
    if segment.ends_with('.') || segment.ends_with(' ') {
        return Err(PathRefusal::TrailingDotOrSpace);
    }
    if segment
        .chars()
        .any(|c| c.is_control() || REFUSED.contains(&c))
    {
        return Err(PathRefusal::RefusedCharacter);
    }
    let stem = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .trim_end()
        .to_lowercase();
    let device = DEVICE_PREFIXES.iter().any(|prefix| {
        stem.strip_prefix(prefix)
            .is_some_and(|digit| DEVICE_DIGITS.contains(&digit))
    });
    if device || RESERVED.contains(&stem.as_str()) {
        return Err(PathRefusal::ReservedName);
    }
    Ok(())
}

#[must_use]
pub(crate) fn fold(rel: &str) -> String {
    rel.nfc().collect::<String>().to_lowercase().nfc().collect()
}

pub(crate) fn case_collision<'a>(
    rel: &str,
    known: impl IntoIterator<Item = &'a str>,
) -> Result<(), PathRefusal> {
    let key = fold(rel);
    known
        .into_iter()
        .find(|other| *other != rel && fold(other) == key)
        .map_or(Ok(()), |existing| {
            Err(PathRefusal::CaseCollision {
                existing: existing.to_owned(),
            })
        })
}

#[must_use]
pub fn is_ignored(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with('.')
        || lower.starts_with("~$")
        || lower.ends_with('~')
        || (lower.starts_with('#') && lower.ends_with('#'))
        || IGNORED_NAMES.contains(&lower.as_str())
        || Path::new(&lower)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| IGNORED_EXTENSIONS.contains(&ext))
}

#[must_use]
pub fn conflict_name(rel: &str, host: &str, attempt: u32) -> String {
    let (dir, file) = rel.rsplit_once('/').unwrap_or(("", rel));
    let (stem, ext) = match file.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, Some(ext)),
        _ => (file, None),
    };
    let host = host_label(host);
    let counter = if attempt > 1 {
        format!(" {attempt}")
    } else {
        String::new()
    };
    let name = ext.map_or_else(
        || format!("{stem} (conflict from {host}){counter}"),
        |ext| format!("{stem} (conflict from {host}){counter}.{ext}"),
    );
    if dir.is_empty() {
        name
    } else {
        format!("{dir}/{name}")
    }
}

#[must_use]
pub(crate) fn host_label(host: &str) -> String {
    safe_label(host, "this device")
}

#[must_use]
pub(crate) fn safe_label(raw: &str, fallback: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | ' ' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned
        .trim()
        .trim_end_matches(['.', ' '])
        .trim_start_matches('.')
        .to_owned();
    if trimmed.is_empty() || portable_segment(&trimmed).is_err() {
        fallback.to_owned()
    } else {
        trimmed
    }
}
