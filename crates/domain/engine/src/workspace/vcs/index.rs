//! A reader for git's index file (versions 2-4): the tracked paths with the
//! size and mtime recorded when they were last staged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::VcsError;

const SIGNATURE: &[u8; 4] = b"DIRC";
const FLAG_ASSUME_VALID: u16 = 0x8000;
const FLAG_EXTENDED: u16 = 0x4000;
const EXT_SKIP_WORKTREE: u16 = 0x4000;
const STAGE_MASK: u16 = 0x3000;
const MODE_TYPE_MASK: u32 = 0o170_000;
pub(crate) const MODE_SYMLINK: u32 = 0o120_000;
pub(crate) const MODE_GITLINK: u32 = 0o160_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexEntry {
    pub(crate) path: String,
    pub(crate) mtime_secs: u32,
    pub(crate) size: u32,
    pub(crate) mode: u32,
    pub(crate) stage: u8,
    pub(crate) assume_valid: bool,
    pub(crate) skip_worktree: bool,
}

impl IndexEntry {
    pub(crate) const fn kind(&self) -> u32 {
        self.mode & MODE_TYPE_MASK
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], VcsError> {
        let end = self.pos.checked_add(len).ok_or(VcsError::CorruptIndex)?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or(VcsError::CorruptIndex)?;
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, VcsError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, VcsError> {
        let raw = self.take(2)?;
        Ok(u16::from_be_bytes([raw[0], raw[1]]))
    }

    fn u32(&mut self) -> Result<u32, VcsError> {
        let raw = self.take(4)?;
        Ok(u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
    }

    fn until_nul(&mut self) -> Result<&'a [u8], VcsError> {
        let rest = self.bytes.get(self.pos..).ok_or(VcsError::CorruptIndex)?;
        let len = rest
            .iter()
            .position(|b| *b == 0)
            .ok_or(VcsError::CorruptIndex)?;
        let slice = self.take(len)?;
        self.pos += 1;
        Ok(slice)
    }

    fn offset_varint(&mut self) -> Result<usize, VcsError> {
        let mut byte = self.u8()?;
        let mut value = usize::from(byte & 0x7f);
        while byte & 0x80 != 0 {
            byte = self.u8()?;
            value = value
                .checked_add(1)
                .and_then(|v| v.checked_mul(128))
                .and_then(|v| v.checked_add(usize::from(byte & 0x7f)))
                .ok_or(VcsError::CorruptIndex)?;
        }
        Ok(value)
    }
}

pub(crate) fn parse(bytes: &[u8], hash_len: usize) -> Result<Vec<IndexEntry>, VcsError> {
    let mut cursor = Cursor { bytes, pos: 0 };
    if cursor.take(4)? != SIGNATURE {
        return Err(VcsError::CorruptIndex);
    }
    let version = cursor.u32()?;
    if !(2..=4).contains(&version) {
        return Err(VcsError::UnsupportedIndex(version));
    }
    let count = cursor.u32()?;
    let mut entries = Vec::with_capacity(usize::try_from(count).unwrap_or(0).min(1 << 20));
    let mut previous: Vec<u8> = Vec::new();

    for _ in 0..count {
        let start = cursor.pos;
        cursor.take(8)?;
        let mtime_secs = cursor.u32()?;
        cursor.take(4 + 4 + 4)?;
        let mode = cursor.u32()?;
        cursor.take(4 + 4)?;
        let size = cursor.u32()?;
        cursor.take(hash_len)?;
        let flags = cursor.u16()?;
        let extended = if version >= 3 && flags & FLAG_EXTENDED != 0 {
            cursor.u16()?
        } else {
            0
        };

        let path = if version == 4 {
            let strip = cursor.offset_varint()?;
            let keep = previous
                .len()
                .checked_sub(strip)
                .ok_or(VcsError::CorruptIndex)?;
            previous.truncate(keep);
            previous.extend_from_slice(cursor.until_nul()?);
            previous.clone()
        } else {
            let name = cursor.until_nul()?.to_vec();
            let consumed = cursor.pos - start;
            let padded = (consumed + 7) & !7;
            cursor.take(padded - consumed)?;
            name
        };

        entries.push(IndexEntry {
            path: String::from_utf8_lossy(&path).into_owned(),
            mtime_secs,
            size,
            mode,
            stage: u8::try_from((flags & STAGE_MASK) >> 12).unwrap_or(0),
            assume_valid: flags & FLAG_ASSUME_VALID != 0,
            skip_worktree: extended & EXT_SKIP_WORKTREE != 0,
        });
    }
    Ok(entries)
}
