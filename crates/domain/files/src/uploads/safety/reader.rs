//! The readers the zip guard's blocking scan reads through: the storage
//! stream as a blocking [`Read`], and a buffered reader that counts the
//! bytes consumed so the scan knows each header's archive offset.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{self, BufRead, Read};

use bytes::Bytes;
use tokio::sync::mpsc;

/// The storage stream as a blocking reader.
pub(super) struct Pipe {
    receiver: mpsc::Receiver<io::Result<Bytes>>,
    current: Bytes,
}

impl Pipe {
    pub(super) const fn new(receiver: mpsc::Receiver<io::Result<Bytes>>) -> Self {
        Self {
            receiver,
            current: Bytes::new(),
        }
    }
}

impl Read for Pipe {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        while self.current.is_empty() {
            match self.receiver.blocking_recv() {
                Some(chunk) => self.current = chunk?,
                None => return Ok(0),
            }
        }
        let n = out.len().min(self.current.len());
        out[..n].copy_from_slice(&self.current.split_to(n));
        Ok(n)
    }
}

/// A buffered reader that counts the bytes consumed, so the scan knows the
/// archive offset of every header it reads.
pub(super) struct Counted<R> {
    inner: R,
    pub(super) position: u64,
}

impl<R: BufRead> Counted<R> {
    pub(super) const fn new(inner: R) -> Self {
        Self { inner, position: 0 }
    }
}

impl<R: BufRead> Read for Counted<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(out)?;
        self.position += n as u64;
        Ok(n)
    }
}

impl<R: BufRead> BufRead for Counted<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        self.inner.consume(amount);
        self.position += amount as u64;
    }
}
