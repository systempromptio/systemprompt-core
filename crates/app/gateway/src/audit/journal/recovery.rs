//! The journal's background passes: adopting the segments a dead process
//! left behind, listing what is ready to settle, and compacting segments.
//!
//! These are the only paths that list the journal directory, and they run
//! from recovery, never on the request path. A process holds the lock on its
//! own `<owner>.owner` file for its whole life, so a segment set whose lock
//! can be taken belongs to a process that has exited: its receipts are
//! replayed into this process's log, a request it never finished is settled
//! as failed, and its files are deleted once the adopted copies are durable.
//! A torn frame ends its segment; it is logged and counted, and because the
//! segment is deleted after adoption it is never read, or replayed, again.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

use systemprompt_identifiers::AiRequestId;

use super::Receipt;
use super::log::{Log, Op, guard};
use super::ops::{MAX_ENTRIES, put_terminal, read};
use super::segment::{self, Entry, Record};
use crate::error::GatewayAuditResult as Result;

pub(super) const ABANDONED: &str = "Accounting incomplete: request ended before a terminal receipt was persisted; upstream usage is unknown";

pub(super) fn settleable(log: &Log) -> Result<Vec<Receipt>> {
    let ready: Vec<_> = guard(&log.state)?
        .slots
        .iter()
        .filter(|(_, slot)| !slot.in_flight)
        .map(|(id, slot)| (id.clone(), *slot))
        .collect();
    let mut receipts = Vec::with_capacity(ready.len());
    for (id, slot) in ready {
        let mut receipt = match read(log, slot.seq, slot.offset, slot.len) {
            Ok(receipt) => receipt,
            Err(error) => {
                tracing::error!(ai_request_id = %id, %error, "Gateway journal receipt unreadable; retained");
                continue;
            },
        };
        if !slot.terminal {
            receipt.failure = Some(ABANDONED.to_owned());
            put_terminal(log, &receipt)?;
        }
        receipts.push(receipt);
    }
    Ok(receipts)
}

pub(super) struct Adoption {
    pub adopted: usize,
    pub torn: u64,
}

pub(super) fn adopt(log: &Log) -> Result<Adoption> {
    let mut owners = Vec::new();
    for entry in fs::read_dir(&log.root)? {
        let path = entry?.path();
        if path.extension().is_some_and(|x| x == segment::OWNER_SUFFIX)
            && let Some(owner) = path.file_stem().and_then(|s| s.to_str())
            && owner != log.owner
        {
            owners.push(owner.to_owned());
        }
    }
    let mut adoption = Adoption {
        adopted: 0,
        torn: 0,
    };
    for owner in owners {
        adoption.adopted += adopt_owner(log, &owner, &mut adoption.torn)?;
    }
    Ok(adoption)
}

fn adopt_owner(log: &Log, owner: &str, torn: &mut u64) -> Result<usize> {
    let lock_path = segment::owner_path(&log.root, owner);
    let lock = match OpenOptions::new().read(true).write(true).open(&lock_path) {
        Ok(lock) => lock,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    match lock.try_lock() {
        Ok(()) => {},
        Err(fs::TryLockError::WouldBlock) => return Ok(0),
        Err(error) => return Err(error.into()),
    }
    if !lock_path.exists() {
        return Ok(0);
    }
    let segments = segments_of(&log.root, owner)?;
    let mut receipts: HashMap<AiRequestId, Receipt> = HashMap::new();
    for path in &segments {
        let scanned = segment::scan(&log.cipher, path)?;
        for entry in scanned.entries {
            match entry {
                Entry::Put(receipt) => {
                    receipts.insert(receipt.storage_id(), *receipt);
                },
                Entry::Remove(id) => {
                    receipts.remove(&id);
                },
            }
        }
        if let Some((offset, cause)) = scanned.torn {
            *torn += 1;
            tracing::error!(
                segment = %path.display(),
                offset,
                error = %cause,
                "Gateway journal segment ends in a torn frame; the bytes after it are skipped"
            );
        }
    }
    let mut adopted = 0;
    for (id, mut receipt) in receipts {
        {
            let state = guard(&log.state)?;
            if state.slots.contains_key(&id) {
                continue;
            }
            if state.slots.len() + state.reserved >= MAX_ENTRIES {
                tracing::warn!(
                    owner,
                    "Gateway journal full; adoption resumes on the next pass"
                );
                return Ok(adopted);
            }
        }
        if receipt.completion.is_none()
            && receipt.failure.is_none()
            && receipt.accounting_failure.is_none()
        {
            receipt.failure = Some(ABANDONED.to_owned());
        }
        let frame = segment::seal(&log.cipher, &Record::Put(&receipt))?;
        log.commit(
            frame,
            Op::Put {
                id,
                terminal: true,
                admit: false,
            },
        )?;
        adopted += 1;
    }
    for path in &segments {
        fs::remove_file(path)?;
    }
    fs::remove_file(&lock_path)?;
    log.sync.sync(&File::open(&log.root)?)?;
    Ok(adopted)
}

fn segments_of(root: &Path, owner: &str) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if let Some((of, seq)) = segment::parse_segment(&path)
            && of == owner
        {
            found.push((seq, path));
        }
    }
    found.sort_by_key(|(seq, _)| *seq);
    Ok(found.into_iter().map(|(_, path)| path).collect())
}

pub(super) fn compact(log: &Log) -> Result<()> {
    let active = guard(&log.writer)?.seq;
    let oldest_live = guard(&log.state)?
        .live
        .iter()
        .find(|(seq, live)| **seq < active && **live > 0)
        .map(|(seq, _)| *seq);
    if let Some(seq) = oldest_live {
        carry_forward(log, seq)?;
    }
    let dead: Vec<u64> = guard(&log.state)?
        .live
        .iter()
        .take_while(|(seq, live)| **seq < active && **live == 0)
        .map(|(seq, _)| *seq)
        .collect();
    if dead.is_empty() {
        return Ok(());
    }
    for seq in &dead {
        match fs::remove_file(segment::segment_path(&log.root, &log.owner, *seq)) {
            Ok(()) => {},
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(error.into()),
        }
        guard(&log.state)?.live.remove(seq);
    }
    log.sync.sync(&File::open(&log.root)?)?;
    Ok(())
}

fn carry_forward(log: &Log, seq: u64) -> Result<()> {
    let held: Vec<_> = guard(&log.state)?
        .slots
        .iter()
        .filter(|(_, slot)| slot.seq == seq)
        .map(|(id, slot)| (id.clone(), *slot))
        .collect();
    for (id, slot) in held {
        let receipt = read(log, slot.seq, slot.offset, slot.len)?;
        let frame = segment::seal(&log.cipher, &Record::Put(&receipt))?;
        log.commit(
            frame,
            Op::Carry {
                id,
                from: (slot.seq, slot.offset),
            },
        )?;
    }
    Ok(())
}

pub(super) fn legacy_receipts(root: &Path) -> Result<usize> {
    let mut count = 0;
    for entry in fs::read_dir(root)? {
        if entry?.path().extension().is_some_and(|x| x == "receipt") {
            count += 1;
        }
    }
    Ok(count)
}
