//! Streaming reads from a [`super::FileStorage`] backend.
//!
//! A read is a byte stream plus the object's metadata. When a [`ByteRange`]
//! is requested and the backend honours it, [`StoredFileRead::range`] says
//! which bytes the stream carries; `None` means the whole object.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::pin::Pin;

use bytes::Bytes;
use futures::Stream;

use super::{FileStorageError, StoredFileMetadata};

pub type FileByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, FileStorageError>> + Send>>;

/// One HTTP-style byte range (`bytes=a-b`, `bytes=a-`, `bytes=-n`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteRange {
    From(u64),
    Bounded { start: u64, end: u64 },
    Suffix(u64),
}

impl ByteRange {
    #[must_use]
    pub fn parse(header: &str) -> Option<Self> {
        let spec = header.trim().strip_prefix("bytes=")?;
        if spec.contains(',') {
            return None;
        }
        let (start, end) = spec.split_once('-')?;
        let (start, end) = (start.trim(), end.trim());
        match (start.is_empty(), end.is_empty()) {
            (true, false) => end.parse().ok().map(Self::Suffix),
            (false, true) => start.parse().ok().map(Self::From),
            (false, false) => {
                let (start, end) = (start.parse().ok()?, end.parse().ok()?);
                (start <= end).then_some(Self::Bounded { start, end })
            },
            (true, true) => None,
        }
    }

    pub fn resolve(self, size: u64) -> Result<ServedRange, FileStorageError> {
        let unsatisfiable = FileStorageError::RangeNotSatisfiable { size };
        let (start, end) = match self {
            Self::From(start) => (start, size.checked_sub(1).ok_or(unsatisfiable)?),
            Self::Bounded { start, end } => (start, end.min(size.saturating_sub(1))),
            Self::Suffix(0) => return Err(unsatisfiable),
            Self::Suffix(n) => (
                size.saturating_sub(n),
                size.checked_sub(1).ok_or(unsatisfiable)?,
            ),
        };
        if start >= size || start > end {
            return Err(FileStorageError::RangeNotSatisfiable { size });
        }
        Ok(ServedRange { start, end, size })
    }

    #[must_use]
    pub fn header_value(self) -> String {
        match self {
            Self::From(start) => format!("bytes={start}-"),
            Self::Bounded { start, end } => format!("bytes={start}-{end}"),
            Self::Suffix(n) => format!("bytes=-{n}"),
        }
    }
}

/// The inclusive byte span a ranged read carries, out of `size`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServedRange {
    pub start: u64,
    pub end: u64,
    pub size: u64,
}

impl ServedRange {
    #[must_use]
    pub const fn length(&self) -> u64 {
        self.end - self.start + 1
    }

    #[must_use]
    pub fn content_range(&self) -> String {
        format!("bytes {}-{}/{}", self.start, self.end, self.size)
    }
}

/// An open object: its metadata, the range served (if any) and its bytes.
pub struct StoredFileRead {
    pub metadata: StoredFileMetadata,
    pub range: Option<ServedRange>,
    pub body: FileByteStream,
}

impl std::fmt::Debug for StoredFileRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredFileRead")
            .field("metadata", &self.metadata)
            .field("range", &self.range)
            .finish_non_exhaustive()
    }
}
