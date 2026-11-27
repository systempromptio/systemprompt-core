//! Resumable uploads: which byte range each missing part carries, and how far a
//! transfer has got.
//!
//! An upload is opened with the whole file's size and SHA-256; the instance
//! answers its part size and the parts it still lacks (numbered from 1).
//! [`plan_parts`] turns that answer into ranges of the caller's bytes, refusing
//! bytes of another length and part numbers outside the upload, so a resumed
//! transfer sends exactly what is missing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Range;

use sp_protocol::wire::drive::DriveUpload;

use crate::error::DriveError;

/// One part to send: its number and the range of the file it carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedPart {
    pub number: u32,
    pub range: Range<usize>,
}

/// How far a transfer has got, reported after each part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadProgress {
    pub sent_bytes: u64,
    pub total_bytes: u64,
    pub parts_sent: u32,
    pub parts_total: u32,
}

pub fn plan_parts(upload: &DriveUpload, data_len: usize) -> Result<Vec<PlannedPart>, DriveError> {
    if u64::try_from(data_len).ok() != Some(upload.size) {
        return Err(DriveError::InvalidInput {
            what: "the bytes are not the ones the upload was opened with",
        });
    }
    if upload.part_size == 0 {
        return Err(DriveError::InvalidInput {
            what: "the instance answered a part size of zero",
        });
    }
    let mut numbers = upload.missing_parts.clone();
    numbers.sort_unstable();
    numbers.dedup();
    numbers
        .into_iter()
        .map(|number| {
            if number == 0 || number > upload.part_count {
                return Err(DriveError::InvalidInput {
                    what: "the instance named a part outside the upload",
                });
            }
            let range = upload.part_range(number).ok_or(DriveError::InvalidInput {
                what: "the instance named a part past the end of the file",
            })?;
            let start =
                usize::try_from(range.start).map_err(|_overflow| DriveError::InvalidInput {
                    what: "a part does not fit in memory",
                })?;
            let end = usize::try_from(range.end).map_err(|_overflow| DriveError::InvalidInput {
                what: "a part does not fit in memory",
            })?;
            Ok(PlannedPart {
                number,
                range: start..end,
            })
        })
        .collect()
}

#[must_use]
pub fn already_sent(upload: &DriveUpload, missing: &[PlannedPart]) -> u64 {
    let pending: u64 = missing
        .iter()
        .map(|part| u64::try_from(part.range.len()).unwrap_or(u64::MAX))
        .fold(0u64, u64::saturating_add);
    upload.size.saturating_sub(pending)
}
