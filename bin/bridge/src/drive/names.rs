//! Whether a gateway path can be a file name on every desktop filesystem.
//!
//! The same Drive folder is mirrored on Windows, macOS and Linux, so a name
//! is placed only when all three can hold it under that exact name: no
//! Windows device name (`CON`, `aux.txt`, `COM1`, `COM¹`, `CONIN$`), no
//! trailing dot or space, none of the characters NTFS refuses. NTFS and APFS
//! compare names without case, and APFS without Unicode normalisation form,
//! so two paths that fold to the same key (NFC, then lowercase, then NFC
//! again) are one file there; the folded key is what collisions are detected
//! on, on every OS alike.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use unicode_normalization::UnicodeNormalization as _;

const RESERVED: [&str; 6] = ["con", "prn", "aux", "nul", "conin$", "conout$"];
const NUMBER_SUFFIXES: [&str; 12] = [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "\u{b9}", "\u{b2}", "\u{b3}",
];
const NUMBERED: [&str; 2] = ["com", "lpt"];
const REFUSED_CHARS: [char; 7] = ['<', '>', '"', '|', '?', '*', ':'];

/// Why a path names no file this device can place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathRefusal {
    ReservedName,
    TrailingDotOrSpace,
    RefusedCharacter,
    Symlink,
    Occupied,
    CaseCollision { existing: String },
}

impl std::fmt::Display for PathRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReservedName => f.write_str("it is a reserved device name on Windows"),
            Self::TrailingDotOrSpace => f.write_str("it ends in a dot or a space"),
            Self::RefusedCharacter => f.write_str("it holds a character Windows refuses"),
            Self::Symlink => f.write_str("it lies under a link that leaves the Drive folder"),
            Self::Occupied => {
                f.write_str("a file this device does not sync already holds the name")
            },
            Self::CaseCollision { existing } => {
                write!(f, "it differs only in case or Unicode form from {existing}")
            },
        }
    }
}

pub fn portable_segment(segment: &str) -> Result<(), PathRefusal> {
    if segment.ends_with('.') || segment.ends_with(' ') {
        return Err(PathRefusal::TrailingDotOrSpace);
    }
    if segment
        .chars()
        .any(|c| c.is_control() || REFUSED_CHARS.contains(&c))
    {
        return Err(PathRefusal::RefusedCharacter);
    }
    let stem = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .trim_end()
        .to_ascii_lowercase();
    let numbered = NUMBERED.iter().any(|prefix| {
        stem.strip_prefix(prefix)
            .is_some_and(|n| NUMBER_SUFFIXES.contains(&n))
    });
    if RESERVED.contains(&stem.as_str()) || numbered {
        return Err(PathRefusal::ReservedName);
    }
    Ok(())
}

#[must_use]
pub fn fold(rel: &str) -> String {
    rel.nfc().collect::<String>().to_lowercase().nfc().collect()
}

pub fn case_collision<'a>(
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
