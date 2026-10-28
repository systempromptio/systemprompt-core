//! The zip-bomb guard for Office and OpenDocument uploads.
//!
//! The archive is read once, front to back, through its local file headers,
//! and every entry is actually inflated into a counter; nothing is written
//! anywhere and the archive is never held in memory. Limits apply to the
//! bytes the entries really expand to, never to the sizes the headers
//! declare, so a lying header gains nothing: the guard refuses more entries
//! than allowed, a running total of decompressed bytes over the limit, or an
//! entry (over 1 MiB) or a whole archive whose real ratio is past the limit.
//! Zip64 sizes are read from the zip64 extra field.
//!
//! The central directory is read first (see `directory`), and the scan
//! holds the local entries to it: each local header must start exactly at
//! the offset its directory entry names, in order, each entry's data must
//! take exactly the compressed bytes the directory names, and the last entry
//! must end where the directory starts. Out-of-order, overlapping or
//! unlisted entries, and gap bytes anywhere between them, are refused, so
//! no bytes of the archive escape the guard. Encrypted entries,
//! compression methods other than stored and deflate, and a stored entry of
//! unknown length are refused. Inflating runs on a blocking thread fed by
//! the storage stream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{self, BufRead, BufReader, Read};

use bytes::Bytes;
use flate2::bufread::DeflateDecoder;
use futures_util::StreamExt;
use systemprompt_traits::{FileStorage, StoredFileId};
use tokio::sync::mpsc;

use super::directory::{self, Directory, refuse};
use crate::config::UploadLimits;
use crate::uploads::error::{UploadError, UploadResult};

const LOCAL_SIGNATURE: u32 = 0x0403_4b50;
const CENTRAL_SIGNATURE: u32 = 0x0201_4b50;
const DESCRIPTOR_SIGNATURE: u32 = 0x0807_4b50;
const ZIP64_EXTRA: u16 = 0x0001;
const FLAG_ENCRYPTED: u16 = 0x0001;
const FLAG_DESCRIPTOR: u16 = 0x0008;
const METHOD_STORED: u16 = 0;
const METHOD_DEFLATE: u16 = 8;
const RATIO_FLOOR_BYTES: u64 = 1024 * 1024;
const READ_BUFFER: usize = 64 * 1024;
const PIPE_DEPTH: usize = 8;

pub(crate) async fn check(
    storage: &dyn FileStorage,
    id: &StoredFileId,
    size: u64,
    limits: &UploadLimits,
) -> UploadResult<()> {
    let expected = directory::read(storage, id, size, limits).await?;
    let mut body = storage.open(id, None).await?.body;
    let (sender, receiver) = mpsc::channel::<io::Result<Bytes>>(PIPE_DEPTH);
    let limits = *limits;
    let scanning = tokio::task::spawn_blocking(move || {
        scan(
            BufReader::with_capacity(READ_BUFFER, Pipe::new(receiver)),
            size,
            &limits,
        )
    });
    while let Some(chunk) = body.next().await {
        if sender.send(chunk.map_err(io::Error::other)).await.is_err() {
            break;
        }
    }
    drop(sender);
    scanning
        .await
        .map_err(|error| refuse(format!("the archive scan stopped: {error}")))?
}

/// The storage stream as a blocking reader.
struct Pipe {
    receiver: mpsc::Receiver<io::Result<Bytes>>,
    current: Bytes,
}

impl Pipe {
    const fn new(receiver: mpsc::Receiver<io::Result<Bytes>>) -> Self {
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
struct Counted<R> {
    inner: R,
    position: u64,
}

impl<R: BufRead> Counted<R> {
    const fn new(inner: R) -> Self {
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

struct Entry {
    flags: u16,
    method: u16,
    compressed: u64,
    zip64: bool,
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    let b = bytes.get(at..at + 8)?;
    Some(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut word = [0u8; 4];
    reader.read_exact(&mut word)?;
    Ok(u32::from_le_bytes(word))
}

fn skip(reader: &mut impl Read, len: u64) -> io::Result<()> {
    let skipped = io::copy(&mut reader.take(len), &mut io::sink())?;
    if skipped == len {
        Ok(())
    } else {
        Err(io::ErrorKind::UnexpectedEof.into())
    }
}

/// The zip64 compressed size, when the 32-bit field defers to the extra.
fn zip64_compressed(extra: &[u8], uncompressed32: u32, compressed32: u32) -> Option<u64> {
    let mut at = 0;
    while at + 4 <= extra.len() {
        let id = u16_at(extra, at);
        let len = usize::from(u16_at(extra, at + 2));
        let data = extra.get(at + 4..at + 4 + len)?;
        if id == ZIP64_EXTRA {
            let skip_uncompressed = if uncompressed32 == u32::MAX { 8 } else { 0 };
            return if compressed32 == u32::MAX {
                u64_at(data, skip_uncompressed)
            } else {
                Some(u64::from(compressed32))
            };
        }
        at += 4 + len;
    }
    None
}

fn local_entry(reader: &mut impl Read) -> io::Result<Entry> {
    let mut header = [0u8; 26];
    reader.read_exact(&mut header)?;
    let flags = u16_at(&header, 2);
    let method = u16_at(&header, 4);
    let compressed32 = u32_at(&header, 14);
    let uncompressed32 = u32_at(&header, 18);
    skip(reader, u64::from(u16_at(&header, 22)))?;
    let mut extra = vec![0u8; usize::from(u16_at(&header, 24))];
    reader.read_exact(&mut extra)?;
    let zip64 = zip64_compressed(&extra, uncompressed32, compressed32);
    Ok(Entry {
        flags,
        method,
        compressed: zip64.unwrap_or_else(|| u64::from(compressed32)),
        zip64: zip64.is_some(),
    })
}

/// Inflates or reads one entry's data into a counter; answers the bytes it
/// expanded to and the bytes it took from the archive.
fn entry_bytes<R: BufRead>(reader: &mut R, entry: &Entry, budget: u64) -> UploadResult<(u64, u64)> {
    let over = |out: u64| out > budget;
    match entry.method {
        METHOD_STORED if entry.flags & FLAG_DESCRIPTOR != 0 => {
            Err(refuse("a stored entry of unknown length is not accepted"))
        },
        METHOD_STORED => {
            if over(entry.compressed) {
                return Err(refuse("entries expand past the decompressed limit"));
            }
            skip(reader, entry.compressed).map_err(unreadable)?;
            Ok((entry.compressed, entry.compressed))
        },
        METHOD_DEFLATE => {
            let mut inflater = DeflateDecoder::new(reader);
            let out = io::copy(&mut (&mut inflater).take(budget + 1), &mut io::sink())
                .map_err(unreadable)?;
            if over(out) {
                return Err(refuse("entries expand past the decompressed limit"));
            }
            Ok((out, inflater.total_in()))
        },
        other => Err(refuse(format!(
            "compression method {other} is not accepted"
        ))),
    }
}

fn unreadable(error: io::Error) -> UploadError {
    refuse(format!("the archive could not be read: {error}"))
}

fn descriptor(reader: &mut impl Read, zip64: bool) -> io::Result<()> {
    let first = read_u32(reader)?;
    let after_crc = if zip64 { 16 } else { 8 };
    let rest = if first == DESCRIPTOR_SIGNATURE {
        4 + after_crc
    } else {
        after_crc
    };
    skip(reader, rest)
}

fn scan<R: BufRead>(
    mut reader: Counted<R>,
    size: u64,
    limits: &UploadLimits,
    directory: &Directory,
) -> UploadResult<()> {
    let mut entries = directory.entries.iter();
    let mut total: u64 = 0;
    loop {
        let at = reader.position;
        match read_u32(&mut reader).map_err(unreadable)? {
            LOCAL_SIGNATURE => {},
            CENTRAL_SIGNATURE if at == directory.offset && entries.len() == 0 => break,
            CENTRAL_SIGNATURE => {
                return Err(refuse(
                    "the local entries do not end where the central directory starts",
                ));
            },
            _ => return Err(refuse("the archive has bytes no entry accounts for")),
        }
        let expected = entries
            .next()
            .ok_or_else(|| refuse("the archive has a local entry its directory does not list"))?;
        if expected.offset != at {
            return Err(refuse(format!(
                "a local entry starts at {at}, not at {} where the directory puts it",
                expected.offset
            )));
        }
        let entry = local_entry(&mut reader).map_err(unreadable)?;
        if entry.flags & FLAG_ENCRYPTED != 0 {
            return Err(refuse("encrypted entries are not accepted"));
        }
        let budget = limits.office_max_uncompressed_bytes.saturating_sub(total);
        let (out, taken) = entry_bytes(&mut reader, &entry, budget)?;
        if taken != expected.compressed {
            return Err(refuse(format!(
                "an entry takes {taken} compressed bytes, not the {} its directory lists",
                expected.compressed
            )));
        }
        if out > RATIO_FLOOR_BYTES && out / taken.max(1) > limits.office_max_ratio {
            return Err(refuse(format!(
                "an entry expands {out} bytes from {taken}, past the {}x ratio",
                limits.office_max_ratio
            )));
        }
        total += out;
        if entry.flags & FLAG_DESCRIPTOR != 0 {
            descriptor(&mut reader, entry.zip64).map_err(unreadable)?;
        }
    }
    if directory.entries.is_empty() {
        return Err(refuse("the archive has no entries"));
    }
    if total > RATIO_FLOOR_BYTES && total / size.max(1) > limits.office_max_ratio {
        return Err(refuse(format!(
            "the archive expands {total} bytes from {size}, past the {}x ratio",
            limits.office_max_ratio
        )));
    }
    Ok(())
}
