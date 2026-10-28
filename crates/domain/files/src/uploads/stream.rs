//! Byte-stream plumbing for uploads: a [`Tally`] that hashes and counts a
//! stream as it passes to storage and stops it at a byte limit, and the
//! concatenation of stored parts into one stream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, Mutex, PoisonError};

use futures_util::{StreamExt, TryStreamExt};
use sha2::{Digest, Sha256};
use systemprompt_identifiers::ContentHash;
use systemprompt_traits::{FileByteStream, FileStorage, FileStorageError, StoredFileId};

#[derive(Debug)]
struct TallyState {
    hasher: Sha256,
    len: u64,
    overflowed: bool,
}

/// The running SHA-256 and length of a stream, shared with the stream.
#[derive(Debug, Clone)]
pub(crate) struct Tally {
    state: Arc<Mutex<TallyState>>,
    limit: u64,
}

impl Tally {
    pub(crate) fn new(limit: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(TallyState {
                hasher: Sha256::new(),
                len: 0,
                overflowed: false,
            })),
            limit,
        }
    }

    /// `body`, counted and hashed into this tally; past the limit it fails
    /// with a validation error and [`Tally::overflowed`] turns true.
    pub(crate) fn wrap(&self, body: FileByteStream) -> FileByteStream {
        let state = Arc::clone(&self.state);
        let limit = self.limit;
        Box::pin(body.map(move |chunk| {
            let chunk = chunk?;
            let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
            state.len = state.len.saturating_add(chunk.len() as u64);
            if state.len > limit {
                state.overflowed = true;
                return Err(FileStorageError::Validation(format!(
                    "the body exceeds {limit} bytes"
                )));
            }
            state.hasher.update(&chunk);
            Ok(chunk)
        }))
    }

    pub(crate) fn len(&self) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len
    }

    pub(crate) fn overflowed(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .overflowed
    }

    pub(crate) fn hash(&self) -> Option<ContentHash> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        ContentHash::try_new(hex::encode(state.hasher.clone().finalize())).ok()
    }
}

/// The stored parts, in order, as one stream; each is opened only when the
/// previous one is exhausted.
pub(crate) fn concatenated(
    storage: Arc<dyn FileStorage>,
    parts: Vec<StoredFileId>,
) -> FileByteStream {
    Box::pin(
        futures_util::stream::iter(parts)
            .then(move |id| {
                let storage = Arc::clone(&storage);
                async move { storage.open(&id, None).await.map(|read| read.body) }
            })
            .try_flatten(),
    )
}

/// A fresh object for one attempt at part `n` of an upload: every attempt
/// gets its own, so a failed re-send never touches an accepted part.
pub(crate) fn part_storage_id(upload: &str, part: u32) -> StoredFileId {
    StoredFileId::new(format!(
        "documents/uploads/{upload}/part-{part:05}-{}",
        uuid::Uuid::new_v4().simple()
    ))
}

/// Where a completed upload is assembled. It sits under `documents/blobs/`
/// because, unless a check rewrites it or the bytes are already stored, the
/// assembled object becomes the blob itself.
pub(crate) fn assembled_storage_id(upload: &str) -> StoredFileId {
    StoredFileId::new(format!("documents/blobs/uploads/{upload}"))
}
