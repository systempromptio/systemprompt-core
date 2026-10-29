//! The central directory of a zip upload, read from its end records by
//! ranged reads: the end-of-central-directory record, and for a zip64
//! archive its locator and zip64 record. Every entry's local header offset
//! and compressed size are kept, so the streaming scan in `office` can
//! check that the local entries tile the archive exactly.
//!
//! The records must close the archive: exactly one end record whose comment
//! runs to the last byte (two such candidates could be read differently by
//! the guard and a codec, so they are refused), the directory ends where the
//! (zip64) end record starts, and the directory is consumed exactly by its
//! entries. Multi-disk archives are refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::{ByteRange, FileStorage, StoredFileId};

use super::read_range;
use crate::config::UploadLimits;
use crate::uploads::error::{RefusalReason, UploadError, UploadResult};

const EOCD_SIGNATURE: u32 = 0x0605_4b50;
const ZIP64_LOCATOR_SIGNATURE: u32 = 0x0706_4b50;
const ZIP64_EOCD_SIGNATURE: u32 = 0x0606_4b50;
const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
const EOCD_LEN: u64 = 22;
const LOCATOR_LEN: u64 = 20;
const ZIP64_EOCD_LEN: u64 = 56;
const CENTRAL_HEADER_LEN: usize = 46;
const MAX_COMMENT: u64 = 0xffff;
const MAX_DIRECTORY_BYTES: u64 = 64 * 1024 * 1024;
const ZIP64_EXTRA: u16 = 0x0001;

pub(super) fn refuse(detail: impl Into<String>) -> UploadError {
    UploadError::refused(RefusalReason::ArchiveLimits, detail)
}

fn le(bytes: &[u8], at: usize, len: usize) -> Option<u64> {
    let b = bytes.get(at..at.checked_add(len)?)?;
    Some(
        b.iter()
            .rev()
            .fold(0, |acc, byte| (acc << 8) | u64::from(*byte)),
    )
}

fn field(bytes: &[u8], at: usize, len: usize) -> UploadResult<u64> {
    le(bytes, at, len).ok_or_else(|| refuse("the central directory is malformed"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Expected {
    pub(super) offset: u64,
    pub(super) compressed: u64,
}

#[derive(Debug)]
pub(super) struct Directory {
    pub(super) entries: Vec<Expected>,
    pub(super) offset: u64,
}

struct End {
    entries: u64,
    size: u64,
    offset: u64,
    starts_at: u64,
}

pub(super) async fn read(
    storage: &dyn FileStorage,
    id: &StoredFileId,
    size: u64,
    limits: &UploadLimits,
) -> UploadResult<Directory> {
    let search = (EOCD_LEN + MAX_COMMENT + LOCATOR_LEN).min(size);
    let tail = read_range(storage, id, ByteRange::Suffix(search)).await?;
    let tail_start = size - tail.len() as u64;
    let end = end_record(storage, id, &tail, tail_start).await?;
    if end.entries > limits.office_max_entries {
        return Err(refuse(format!(
            "{} entries exceed the {}-entry limit",
            end.entries, limits.office_max_entries
        )));
    }
    let directory_end = end.offset.checked_add(end.size);
    if end.size == 0 || end.size > MAX_DIRECTORY_BYTES || directory_end != Some(end.starts_at) {
        return Err(refuse(
            "the central directory does not end where the end record starts",
        ));
    }
    let central = read_range(
        storage,
        id,
        ByteRange::Bounded {
            start: end.offset,
            end: end.starts_at - 1,
        },
    )
    .await?;
    let mut entries = parse_entries(&central, end.entries)?;
    entries.sort_by_key(|entry| entry.offset);
    Ok(Directory {
        entries,
        offset: end.offset,
    })
}

async fn end_record(
    storage: &dyn FileStorage,
    id: &StoredFileId,
    tail: &[u8],
    tail_start: u64,
) -> UploadResult<End> {
    let candidates: Vec<usize> = (0..tail.len().saturating_sub(EOCD_LEN as usize - 1))
        .filter(|&at| {
            le(tail, at, 4) == Some(u64::from(EOCD_SIGNATURE))
                && le(tail, at + 20, 2)
                    .is_some_and(|comment| at as u64 + EOCD_LEN + comment == tail.len() as u64)
        })
        .collect();
    let at = match candidates.as_slice() {
        [at] => *at,
        [] => return Err(refuse("the archive has no end-of-central-directory record")),
        _ => {
            return Err(refuse(
                "the archive has more than one end-of-central-directory record",
            ));
        },
    };
    let record = &tail[at..];
    if field(record, 4, 2)? != 0 || field(record, 6, 2)? != 0 {
        return Err(refuse("multi-disk archives are not accepted"));
    }
    let entries = field(record, 10, 2)?;
    let size = field(record, 12, 4)?;
    let offset = field(record, 16, 4)?;
    let eocd_at = tail_start + at as u64;
    if entries != 0xffff && size != 0xffff_ffff && offset != 0xffff_ffff {
        return Ok(End {
            entries,
            size,
            offset,
            starts_at: eocd_at,
        });
    }
    let locator_at = at
        .checked_sub(LOCATOR_LEN as usize)
        .ok_or_else(|| refuse("the zip64 locator is missing"))?;
    let locator = &tail[locator_at..at];
    if field(locator, 0, 4)? != u64::from(ZIP64_LOCATOR_SIGNATURE) {
        return Err(refuse("the zip64 locator is missing"));
    }
    let zip64_at = field(locator, 8, 8)?;
    if zip64_at.checked_add(ZIP64_EOCD_LEN) != Some(tail_start + locator_at as u64) {
        return Err(refuse("the zip64 end record is not where its locator says"));
    }
    let zip64 = read_range(
        storage,
        id,
        ByteRange::Bounded {
            start: zip64_at,
            end: zip64_at + ZIP64_EOCD_LEN - 1,
        },
    )
    .await?;
    if field(&zip64, 0, 4)? != u64::from(ZIP64_EOCD_SIGNATURE) || field(&zip64, 4, 8)? != 44 {
        return Err(refuse("the zip64 end record is malformed"));
    }
    Ok(End {
        entries: field(&zip64, 32, 8)?,
        size: field(&zip64, 40, 8)?,
        offset: field(&zip64, 48, 8)?,
        starts_at: zip64_at,
    })
}

fn zip64_values(extra: &[u8]) -> Option<&[u8]> {
    let mut at = 0;
    while at + 4 <= extra.len() {
        let id = le(extra, at, 2)?;
        let len = usize::try_from(le(extra, at + 2, 2)?).ok()?;
        let data = extra.get(at + 4..at + 4 + len)?;
        if id == u64::from(ZIP64_EXTRA) {
            return Some(data);
        }
        at += 4 + len;
    }
    None
}

fn parse_entries(central: &[u8], count: u64) -> UploadResult<Vec<Expected>> {
    let mut entries = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
    let mut at = 0usize;
    for _ in 0..count {
        if field(central, at, 4)? != u64::from(CENTRAL_SIGNATURE) {
            return Err(refuse("the central directory is malformed"));
        }
        let mut compressed = field(central, at + 20, 4)?;
        let uncompressed = field(central, at + 24, 4)?;
        let name_len = usize::try_from(field(central, at + 28, 2)?).unwrap_or(usize::MAX);
        let extra_len = usize::try_from(field(central, at + 30, 2)?).unwrap_or(usize::MAX);
        let comment_len = usize::try_from(field(central, at + 32, 2)?).unwrap_or(usize::MAX);
        let mut offset = field(central, at + 42, 4)?;
        let extra_at = at + CENTRAL_HEADER_LEN + name_len;
        let extra = central
            .get(extra_at..extra_at + extra_len)
            .ok_or_else(|| refuse("the central directory is malformed"))?;
        if compressed == 0xffff_ffff || uncompressed == 0xffff_ffff || offset == 0xffff_ffff {
            let values = zip64_values(extra)
                .ok_or_else(|| refuse("a zip64 entry has no zip64 extra field"))?;
            let mut next = 0;
            let mut take = |present: bool| -> UploadResult<Option<u64>> {
                if !present {
                    return Ok(None);
                }
                let value = field(values, next, 8)?;
                next += 8;
                Ok(Some(value))
            };
            take(uncompressed == 0xffff_ffff)?;
            compressed = take(compressed == 0xffff_ffff)?.unwrap_or(compressed);
            offset = take(offset == 0xffff_ffff)?.unwrap_or(offset);
        }
        entries.push(Expected { offset, compressed });
        at = extra_at + extra_len + comment_len;
    }
    if at != central.len() {
        return Err(refuse(
            "the central directory has bytes no entry accounts for",
        ));
    }
    Ok(entries)
}
