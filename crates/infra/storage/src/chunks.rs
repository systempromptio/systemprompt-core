//! Fixed-size chunks of a byte stream, for the backends that write one
//! object in parts: S3 multipart uploads and Cloud Storage resumable
//! uploads. Only the chunk being sent is held in memory.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures::StreamExt;
use systemprompt_traits::{FileByteStream, FileStorageError};

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

    pub(crate) async fn next(&mut self) -> Result<Option<(Vec<u8>, bool)>, FileStorageError> {
        let Some(current) = self.next.take() else {
            return Ok(None);
        };
        self.next = self.chunks.next().await?;
        Ok(Some((current, self.next.is_none())))
    }
}
