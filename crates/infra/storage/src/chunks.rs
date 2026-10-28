//! Fixed-size chunks of a byte stream, for the backends that write one
//! object in parts: S3 multipart uploads and Cloud Storage resumable
//! uploads. Only the chunk being sent is held in memory.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures::StreamExt;
use systemprompt_traits::{FileByteStream, FileStorageError};

/// The part size both multipart backends send: above S3's 5 MiB minimum
/// part and a multiple of Cloud Storage's 256 KiB chunk granularity.
pub(crate) const CHUNK_BYTES: usize = 8 * 1024 * 1024;

pub(crate) struct Chunks {
    body: FileByteStream,
    pending: Vec<u8>,
    finished: bool,
}

impl Chunks {
    pub(crate) fn new(body: FileByteStream) -> Self {
        Self {
            body,
            pending: Vec::new(),
            finished: false,
        }
    }

    /// The next chunk of exactly [`CHUNK_BYTES`], or the shorter remainder
    /// at the end of the stream; `None` once the stream is exhausted.
    pub(crate) async fn next(&mut self) -> Result<Option<Vec<u8>>, FileStorageError> {
        while !self.finished && self.pending.len() < CHUNK_BYTES {
            match self.body.next().await {
                Some(chunk) => self.pending.extend_from_slice(&chunk?),
                None => self.finished = true,
            }
        }
        if self.pending.is_empty() {
            return Ok(None);
        }
        let split = self.pending.len().min(CHUNK_BYTES);
        let rest = self.pending.split_off(split);
        Ok(Some(std::mem::replace(&mut self.pending, rest)))
    }
}

/// The chunks of a stream with one chunk of lookahead, so the sender knows
/// which one is the last.
pub(crate) struct Lookahead {
    chunks: Chunks,
    next: Option<Vec<u8>>,
}

impl Lookahead {
    pub(crate) async fn new(body: FileByteStream) -> Result<Self, FileStorageError> {
        let mut chunks = Chunks::new(body);
        let next = chunks.next().await?;
        Ok(Self { chunks, next })
    }

    /// The next chunk and whether it is the last one.
    pub(crate) async fn next(&mut self) -> Result<Option<(Vec<u8>, bool)>, FileStorageError> {
        let Some(current) = self.next.take() else {
            return Ok(None);
        };
        self.next = self.chunks.next().await?;
        Ok(Some((current, self.next.is_none())))
    }
}
