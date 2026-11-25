//! Re-keying journal segments sealed before 0.88.
//!
//! Releases before 0.88 sealed every journal frame directly under the
//! `encryption_master_key`; 0.88 seals them under the `gateway` member's
//! journal key. A segment whose first frame opens under neither is torn and
//! left to recovery as before. A segment whose first frame opens under the
//! pre-0.88 scheme still holds receipts that were never settled, so it is
//! never treated as torn: the journal refuses to open while one is present
//! ([`GatewayAtRestError::PreUpgradeJournal`](crate::GatewayAtRestError)),
//! and `systemprompt admin secrets reseal --member gateway --apply` rewrites
//! it frame by frame under the journal key (to a sibling file, synced, then
//! renamed over the segment), so recovery settles its receipts on the next
//! boot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::io::Write;
use std::path::Path;

use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use systemprompt_security::at_rest::{MasterKey, pre_088};

use super::segment::{self, frames, seal_plain, split_body};
use crate::error::GatewayAuditResult as Result;

/// The segments of one journal directory, judged by the scheme their first
/// frame opens under.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JournalReseal {
    pub segments: u64,
    pub current: u64,
    pub pre_088: u64,
    pub resealed: u64,
    pub undecodable: u64,
}

enum Judged {
    Current,
    Pre088(Vec<Vec<u8>>),
    Undecodable,
}

fn judge(current: &ChaCha20Poly1305, master: &MasterKey, bytes: &[u8]) -> Judged {
    let framed = frames(bytes);
    let Some((_, first)) = framed.bodies.first() else {
        return Judged::Current;
    };
    let Some((nonce, sealed)) = split_body(first) else {
        return Judged::Undecodable;
    };
    if current.decrypt(&Nonce::from(nonce), sealed).is_ok() {
        return Judged::Current;
    }
    let mut plains = Vec::new();
    for (_, body) in &framed.bodies {
        let Some((nonce, sealed)) = split_body(body) else {
            break;
        };
        match pre_088::open(master, &nonce, sealed) {
            Ok(plain) => plains.push(plain),
            Err(_torn) => break,
        }
    }
    if plains.is_empty() {
        Judged::Undecodable
    } else {
        Judged::Pre088(plains)
    }
}

fn rewrite(path: &Path, current: &ChaCha20Poly1305, plains: &[Vec<u8>]) -> Result<()> {
    let staged = path.with_extension("reseal");
    let mut file = segment::create(&staged)?;
    for plain in plains {
        file.write_all(&seal_plain(current, plain)?)?;
    }
    file.sync_all()?;
    drop(file);
    fs::rename(&staged, path)?;
    if let Some(dir) = path.parent() {
        fs::File::open(dir)?.sync_all()?;
    }
    Ok(())
}

pub(crate) fn reseal(
    root: &Path,
    current: &ChaCha20Poly1305,
    master: &MasterKey,
    apply: bool,
) -> Result<JournalReseal> {
    let mut report = JournalReseal::default();
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(report),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let path = entry?.path();
        if segment::parse_segment(&path).is_none() {
            continue;
        }
        report.segments += 1;
        match judge(current, master, &fs::read(&path)?) {
            Judged::Current => report.current += 1,
            Judged::Undecodable => report.undecodable += 1,
            Judged::Pre088(plains) => {
                report.pre_088 += 1;
                if apply {
                    rewrite(&path, current, &plains)?;
                    report.resealed += 1;
                }
            },
        }
    }
    Ok(report)
}
