//! The journal's in-memory index: where each retained receipt's latest frame
//! lives, and how many live receipts each segment still holds.
//!
//! A flushed batch is applied here only after its sync returns, so the index
//! never points at a frame that is not durable. A segment whose live count
//! reaches zero is dead and compaction deletes it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, HashMap};

use systemprompt_identifiers::AiRequestId;

#[derive(Debug, Clone, Copy)]
pub(super) struct Slot {
    pub seq: u64,
    pub offset: u64,
    pub len: u32,
    pub terminal: bool,
    pub in_flight: bool,
}

#[derive(Debug, Default)]
pub(super) struct State {
    pub slots: HashMap<AiRequestId, Slot>,
    pub live: BTreeMap<u64, usize>,
    pub reserved: usize,
    pub in_flight: usize,
}

#[derive(Debug)]
pub(super) enum Op {
    Put {
        id: AiRequestId,
        terminal: bool,
        admit: bool,
    },
    Carry {
        id: AiRequestId,
        from: (u64, u64),
    },
    Remove(AiRequestId),
}

pub(super) fn apply(state: &mut State, op: Op, seq: u64, offset: u64, len: u32) {
    match op {
        Op::Put {
            id,
            terminal,
            admit,
        } => {
            let previous = state.slots.get(&id).copied();
            if admit {
                state.reserved = state.reserved.saturating_sub(1);
            }
            let in_flight = admit || (!terminal && previous.is_some_and(|p| p.in_flight));
            if previous.is_some_and(|p| p.in_flight) && !in_flight {
                state.in_flight = state.in_flight.saturating_sub(1);
            }
            if let Some(previous) = previous {
                release(state, previous.seq);
            }
            *state.live.entry(seq).or_default() += 1;
            state.slots.insert(
                id,
                Slot {
                    seq,
                    offset,
                    len,
                    terminal,
                    in_flight,
                },
            );
        },
        Op::Carry { id, from } => {
            let Some(slot) = state.slots.get_mut(&id) else {
                return;
            };
            if (slot.seq, slot.offset) != from {
                return;
            }
            let previous = slot.seq;
            slot.seq = seq;
            slot.offset = offset;
            slot.len = len;
            release(state, previous);
            *state.live.entry(seq).or_default() += 1;
        },
        Op::Remove(id) => {
            if let Some(previous) = state.slots.remove(&id) {
                if previous.in_flight {
                    state.in_flight = state.in_flight.saturating_sub(1);
                }
                release(state, previous.seq);
            }
        },
    }
}

fn release(state: &mut State, seq: u64) {
    if let Some(live) = state.live.get_mut(&seq) {
        *live = live.saturating_sub(1);
    }
}
