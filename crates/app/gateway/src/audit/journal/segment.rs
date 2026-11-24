//! The on-disk segment format of the gateway journal.
//!
//! A segment is an append-only file of frames. A frame is a little-endian
//! `u32` body length followed by the body: a fresh 12-byte nonce and the
//! ChaCha20-Poly1305 sealing of one JSON [`Entry`]. The latest entry for a
//! storage id wins: a `Put` stores a receipt, a `Remove` retires it. A
//! frame that is cut short or does not authenticate ends the segment: every
//! write after a failed flush goes to a fresh segment, so nothing valid ever
//! follows a torn frame.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::AiRequestId;

use super::Receipt;
use crate::error::{GatewayAuditError, GatewayAuditResult as Result, ensure};

pub(super) const SEGMENT_SUFFIX: &str = "seg";
pub(super) const OWNER_SUFFIX: &str = "owner";
pub(super) const ROLL_BYTES: u64 = 4 * 1024 * 1024;
const MAX_PLAIN: usize = 16 * 1024 * 1024;
const NONCE: usize = 12;
const MAX_BODY: usize = MAX_PLAIN + NONCE + 16;

#[derive(Debug, Deserialize)]
pub(super) enum Entry {
    Put(Box<Receipt>),
    Remove(AiRequestId),
}

#[derive(Debug, Serialize)]
pub(super) enum Record<'a> {
    Put(&'a Receipt),
    Remove(&'a AiRequestId),
}

pub(super) fn segment_path(root: &Path, owner: &str, seq: u64) -> PathBuf {
    root.join(format!("{owner}-{seq:010}.{SEGMENT_SUFFIX}"))
}

pub(super) fn owner_path(root: &Path, owner: &str) -> PathBuf {
    root.join(format!("{owner}.{OWNER_SUFFIX}"))
}

pub(super) fn parse_segment(path: &Path) -> Option<(String, u64)> {
    if path.extension().is_none_or(|x| x != SEGMENT_SUFFIX) {
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    let (owner, seq) = stem.rsplit_once('-')?;
    Some((owner.to_owned(), seq.parse().ok()?))
}

pub(super) fn create(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.append(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

pub(super) fn seal(cipher: &ChaCha20Poly1305, entry: &Record<'_>) -> Result<Vec<u8>> {
    let plain = serde_json::to_vec(entry)?;
    ensure(
        plain.len() <= MAX_PLAIN,
        "Gateway completion exceeds journal capacity",
    )?;
    let nonce: [u8; NONCE] = rand::random();
    let sealed = cipher
        .encrypt(&Nonce::from(nonce), plain.as_slice())
        .map_err(GatewayAuditError::Encrypt)?;
    let body = u32::try_from(NONCE + sealed.len())
        .map_err(|_overflow| GatewayAuditError::Invariant("Journal frame too large"))?;
    let mut frame = Vec::with_capacity(4 + NONCE + sealed.len());
    frame.extend_from_slice(&body.to_le_bytes());
    frame.extend_from_slice(&nonce);
    frame.extend_from_slice(&sealed);
    Ok(frame)
}

fn open_body(cipher: &ChaCha20Poly1305, body: &[u8]) -> Result<Entry> {
    ensure(body.len() > NONCE, "Truncated journal frame")?;
    let (nonce, sealed) = body.split_at(NONCE);
    let mut fixed = [0u8; NONCE];
    fixed.copy_from_slice(nonce);
    let plain = cipher
        .decrypt(&Nonce::from(fixed), sealed)
        .map_err(GatewayAuditError::Authenticate)?;
    Ok(serde_json::from_slice(&plain)?)
}

pub(super) fn read_at(
    cipher: &ChaCha20Poly1305,
    path: &Path,
    offset: u64,
    len: u32,
) -> Result<Entry> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut body = vec![0u8; len as usize];
    file.read_exact(&mut body)?;
    open_body(cipher, &body)
}

#[derive(Debug)]
pub(super) struct Scanned {
    pub entries: Vec<Entry>,
    pub torn: Option<(u64, GatewayAuditError)>,
}

pub(super) fn scan(cipher: &ChaCha20Poly1305, path: &Path) -> Result<Scanned> {
    let bytes = fs::read(path)?;
    let mut entries = Vec::new();
    let mut at = 0usize;
    while at < bytes.len() {
        let Some(header) = bytes.get(at..at + 4) else {
            return Ok(torn(entries, at, "Journal frame header cut short"));
        };
        let mut raw = [0u8; 4];
        raw.copy_from_slice(header);
        let body = u32::from_le_bytes(raw) as usize;
        let Some(frame) = bytes
            .get(at + 4..at + 4 + body)
            .filter(|_| body <= MAX_BODY)
        else {
            return Ok(torn(entries, at, "Journal frame body cut short"));
        };
        match open_body(cipher, frame) {
            Ok(entry) => entries.push(entry),
            Err(error) => {
                return Ok(Scanned {
                    entries,
                    torn: Some((at as u64, error)),
                });
            },
        }
        at += 4 + body;
    }
    Ok(Scanned {
        entries,
        torn: None,
    })
}

const fn torn(entries: Vec<Entry>, at: usize, why: &'static str) -> Scanned {
    Scanned {
        entries,
        torn: Some((at as u64, GatewayAuditError::Invariant(why))),
    }
}
