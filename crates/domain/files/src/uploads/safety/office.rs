//! The zip-bomb guard for Office and OpenDocument uploads.
//!
//! The guard reads only the archive's end-of-central-directory record and
//! its central directory, by ranged reads, so it never holds the archive in
//! memory. It refuses an archive with more entries than allowed, a total
//! declared uncompressed size over the limit, or an entry (over 1 MiB)
//! whose compression ratio is past the limit. Zip64 archives are refused:
//! their sizes live in extra fields this guard does not trust. Sizes are as
//! the archive declares them; the import codec decompresses under its own
//! input limits, so a lying header is caught there.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::{ByteRange, FileStorage, StoredFileId};

use super::read_range;
use crate::config::UploadLimits;
use crate::uploads::error::{RefusalReason, UploadError, UploadResult};

const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
const CENTRAL_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
const EOCD_LEN: usize = 22;
const EOCD_SEARCH: u64 = (EOCD_LEN as u64) + 0xffff;
const CENTRAL_HEADER_LEN: usize = 46;
const RATIO_FLOOR_BYTES: u64 = 1024 * 1024;
const MAX_DIRECTORY_BYTES: u64 = 64 * 1024 * 1024;

fn refuse(detail: impl Into<String>) -> UploadError {
    UploadError::refused(RefusalReason::ArchiveLimits, detail)
}

fn u16_at(bytes: &[u8], at: usize) -> u64 {
    bytes
        .get(at..at + 2)
        .map_or(0, |b| u64::from(u16::from_le_bytes([b[0], b[1]])))
}

fn u32_at(bytes: &[u8], at: usize) -> u64 {
    bytes.get(at..at + 4).map_or(0, |b| {
        u64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    })
}

struct Directory {
    entries: u64,
    size: u64,
    offset: u64,
}

fn directory(tail: &[u8]) -> UploadResult<Directory> {
    let at = tail
        .windows(4)
        .rposition(|window| window == EOCD_SIGNATURE)
        .filter(|at| tail.len() - at >= EOCD_LEN)
        .ok_or_else(|| refuse("the archive has no end-of-central-directory record"))?;
    let record = &tail[at..];
    let entries = u16_at(record, 10);
    let size = u32_at(record, 12);
    let offset = u32_at(record, 16);
    if entries == 0xffff || size == 0xffff_ffff || offset == 0xffff_ffff {
        return Err(refuse("zip64 archives are not accepted"));
    }
    Ok(Directory {
        entries,
        size,
        offset,
    })
}

pub(crate) async fn check(
    storage: &dyn FileStorage,
    id: &StoredFileId,
    size: u64,
    limits: &UploadLimits,
) -> UploadResult<()> {
    let tail = read_range(storage, id, ByteRange::Suffix(EOCD_SEARCH.min(size))).await?;
    let found = directory(&tail)?;
    if found.entries > limits.office_max_entries {
        return Err(refuse(format!(
            "{} entries exceed the {}-entry limit",
            found.entries, limits.office_max_entries
        )));
    }
    if found.size == 0 || found.size > MAX_DIRECTORY_BYTES || found.offset + found.size > size {
        return Err(refuse("the central directory is out of bounds"));
    }
    let range = ByteRange::Bounded {
        start: found.offset,
        end: found.offset + found.size - 1,
    };
    let central = read_range(storage, id, range).await?;
    walk(&central, found.entries, size, limits)
}

fn walk(central: &[u8], entries: u64, size: u64, limits: &UploadLimits) -> UploadResult<()> {
    let mut at = 0usize;
    let mut total: u64 = 0;
    for _ in 0..entries {
        let header = central
            .get(at..at.saturating_add(CENTRAL_HEADER_LEN))
            .filter(|header| header[..4] == CENTRAL_SIGNATURE)
            .ok_or_else(|| refuse("the central directory is malformed"))?;
        let compressed = u32_at(header, 20);
        let uncompressed = u32_at(header, 24);
        if compressed == 0xffff_ffff || uncompressed == 0xffff_ffff {
            return Err(refuse("zip64 entries are not accepted"));
        }
        if uncompressed > RATIO_FLOOR_BYTES
            && uncompressed / compressed.max(1) > limits.office_max_ratio
        {
            return Err(refuse(format!(
                "an entry expands {uncompressed} bytes from {compressed}, past the {}x ratio",
                limits.office_max_ratio
            )));
        }
        total = total.saturating_add(uncompressed);
        if total > limits.office_max_uncompressed_bytes {
            return Err(refuse(format!(
                "entries expand past the {}-byte limit",
                limits.office_max_uncompressed_bytes
            )));
        }
        let variable = u16_at(header, 28) + u16_at(header, 30) + u16_at(header, 32);
        at = at
            .saturating_add(CENTRAL_HEADER_LEN)
            .saturating_add(usize::try_from(variable).unwrap_or(usize::MAX));
    }
    if total > RATIO_FLOOR_BYTES && total / size.max(1) > limits.office_max_ratio {
        return Err(refuse(format!(
            "the archive expands {total} bytes from {size}, past the {}x ratio",
            limits.office_max_ratio
        )));
    }
    Ok(())
}
