//! The bounded copy of a tapped stream's client bytes kept for the audit
//! payload and the received-surface scan.
//!
//! Every byte the stream delivers is counted, but only the first
//! [`MAX_RETAINED_STREAM_BYTES`] are kept, so a pathological upstream cannot
//! grow the tap's memory without limit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::{Bytes, BytesMut};

pub const MAX_RETAINED_STREAM_BYTES: usize = 8 * 1024 * 1024;

/// The retained prefix of a stream and the total length it was cut from.
#[derive(Debug, Default)]
pub struct RetainedBytes {
    kept: BytesMut,
    streamed: u64,
}

impl RetainedBytes {
    pub fn push(&mut self, bytes: &[u8]) {
        self.streamed = self
            .streamed
            .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        let room = MAX_RETAINED_STREAM_BYTES.saturating_sub(self.kept.len());
        self.kept.extend_from_slice(&bytes[..bytes.len().min(room)]);
    }

    #[must_use]
    pub const fn streamed(&self) -> u64 {
        self.streamed
    }

    #[must_use]
    pub fn retained(&self) -> usize {
        self.kept.len()
    }

    pub fn take(&mut self) -> Bytes {
        std::mem::take(&mut self.kept).freeze()
    }
}
