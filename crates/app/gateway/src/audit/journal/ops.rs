//! The journal's writes, each acknowledged only once its batch is durable.
//!
//! The in-memory index answers every admission check, so the request path
//! never lists the journal directory. Admission counts the requests in
//! flight against the configured limit and refuses past it with
//! [`GatewayAuditError::AdmissionLimit`]; the disk bound counts every
//! retained receipt.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::num::NonZeroU32;
use std::sync::Arc;

use systemprompt_identifiers::AiRequestId;

use super::Receipt;
use super::log::{Log, Op, guard};
use super::segment::{self, Entry, Record};
use crate::error::{GatewayAuditError, GatewayAuditResult as Result, ensure};

// Why: a receipt is held for the whole life of an in-flight request and for
// as long as its settlement keeps failing, so this bounds the disk and the
// index against a runaway backlog; the admission limit is the concurrency
// ceiling.
pub(super) const MAX_ENTRIES: usize = 4096;

#[derive(Debug)]
pub(crate) struct JournalLease {
    log: Arc<Log>,
    id: AiRequestId,
}

impl Drop for JournalLease {
    fn drop(&mut self) {
        let Ok(mut state) = self.log.state.lock() else {
            return;
        };
        let released = state.slots.get_mut(&self.id).is_some_and(|slot| {
            let held = slot.in_flight;
            slot.in_flight = false;
            held
        });
        if released {
            state.in_flight = state.in_flight.saturating_sub(1);
        }
    }
}

pub(super) fn reserve(
    log: &Arc<Log>,
    receipt: &Receipt,
    limit: NonZeroU32,
) -> Result<JournalLease> {
    let id = receipt.request_id.clone();
    {
        let mut state = guard(&log.state)?;
        ensure(!state.slots.contains_key(&id), "Request already admitted")?;
        ensure(
            state.slots.len() + state.reserved < MAX_ENTRIES,
            "Gateway journal capacity exhausted",
        )?;
        if state.in_flight >= limit.get() as usize {
            metrics::counter!(super::JOURNAL_ADMISSIONS_SHED_TOTAL).increment(1);
            return Err(GatewayAuditError::AdmissionLimit { limit: limit.get() });
        }
        state.reserved += 1;
        state.in_flight += 1;
    }
    let committed = segment::seal(&log.cipher, &Record::Put(receipt)).and_then(|frame| {
        log.commit(
            frame,
            Op::Put {
                id: id.clone(),
                terminal: false,
                admit: true,
            },
        )
    });
    if let Err(error) = committed {
        unreserve(&mut *guard(&log.state)?);
        return Err(error);
    }
    Ok(JournalLease {
        log: Arc::clone(log),
        id,
    })
}

const fn unreserve(state: &mut super::log::State) {
    state.reserved = state.reserved.saturating_sub(1);
    state.in_flight = state.in_flight.saturating_sub(1);
}

pub(super) fn put_terminal(log: &Log, receipt: &Receipt) -> Result<()> {
    let terminal = guard(&log.state)?
        .slots
        .get(&receipt.request_id)
        .map(|slot| slot.terminal)
        .ok_or(GatewayAuditError::Invariant(
            "Missing durable admission receipt",
        ))?;
    ensure(
        !terminal,
        "Terminal accounting receipt already exists; recovery will settle it",
    )?;
    let frame = segment::seal(&log.cipher, &Record::Put(receipt))?;
    log.commit(
        frame,
        Op::Put {
            id: receipt.request_id.clone(),
            terminal: true,
            admit: false,
        },
    )
}

pub(super) fn put_accounting_failure(log: &Log, receipt: &Receipt) -> Result<()> {
    ensure(
        receipt
            .accounting_failure
            .as_ref()
            .is_some_and(|error| !error.is_empty() && error.len() <= 4096)
            && receipt.completion.is_none()
            && receipt.failure.is_none(),
        "Invalid accounting failure receipt",
    )?;
    let id = receipt.storage_id();
    let retained = {
        let state = guard(&log.state)?;
        if !state.slots.contains_key(&id) {
            ensure(
                state.slots.len() + state.reserved < MAX_ENTRIES,
                "Gateway accounting journal is full",
            )?;
        }
        state.slots.get(&id).copied()
    };
    if let Some(slot) = retained {
        let previous = read(log, slot.seq, slot.offset, slot.len)?;
        ensure(
            previous.request_id == receipt.request_id
                && previous.user_id == receipt.user_id
                && previous.accounting_failure == receipt.accounting_failure,
            "Conflicting retained accounting failure",
        )?;
        return Ok(());
    }
    let frame = segment::seal(&log.cipher, &Record::Put(receipt))?;
    log.commit(
        frame,
        Op::Put {
            id,
            terminal: true,
            admit: false,
        },
    )
}

pub(super) fn remove(log: &Log, id: &AiRequestId) -> Result<()> {
    if !guard(&log.state)?.slots.contains_key(id) {
        return Ok(());
    }
    let frame = segment::seal(&log.cipher, &Record::Remove(id))?;
    log.commit(frame, Op::Remove(id.clone()))
}

pub(super) fn count(log: &Log) -> Result<usize> {
    Ok(guard(&log.state)?.slots.len())
}

pub(super) fn read(log: &Log, seq: u64, offset: u64, len: u32) -> Result<Receipt> {
    let path = segment::segment_path(&log.root, &log.owner, seq);
    match segment::read_at(&log.cipher, &path, offset, len)? {
        Entry::Put(receipt) => Ok(*receipt),
        Entry::Remove(_) => Err(GatewayAuditError::Invariant(
            "Journal index points at a removal",
        )),
    }
}
