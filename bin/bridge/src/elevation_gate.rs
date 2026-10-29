//! One administrator approval at a time, for the newest content.
//!
//! Every machine-policy change that needs elevation passes this gate. At most
//! one elevated write runs per process; a request that arrives while one is in
//! flight waits for it. Every request stages its content with a sequence
//! number, and a run applies the newest staged content, so it covers every
//! request staged before it: such a request takes that run's result instead of
//! prompting again, and is answered only once a run covering it has finished. A
//! declined prompt suppresses further prompts for that content until the user
//! asks again or the content changes, and an unattended caller never prompts —
//! it records the content as pending for the GUI to offer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

/// Who is asking for the elevated write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requester {
    UserAction,
    Attended,
    Unattended,
}

/// How an elevated write ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateOutcome {
    Applied,
    Declined,
    Failed,
}

/// What the gate tells a caller to do.
#[derive(Debug)]
pub enum Admission<'a> {
    Run(Ticket<'a>),
    Joined(GateOutcome),
    Suppressed,
    Queued,
}

#[derive(Debug, Default)]
struct State {
    staged: u64,
    in_flight: Option<(String, u64)>,
    pending: Option<(String, u64)>,
    declined: Option<String>,
    last: Option<Finished>,
}

#[derive(Debug, Clone)]
struct Finished {
    covers: u64,
    outcome: GateOutcome,
}

impl State {
    fn stage(&mut self, content: &str) -> u64 {
        self.staged += 1;
        self.pending = Some((content.to_owned(), self.staged));
        self.staged
    }
}

/// The per-process elevation gate.
#[derive(Debug, Default)]
pub struct ElevationGate {
    state: Mutex<State>,
    done: Condvar,
}

impl ElevationGate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn request(&self, content: &str, requester: Requester) -> Admission<'_> {
        let mut state = self.lock();
        if requester == Requester::UserAction && state.declined.as_deref() == Some(content) {
            state.declined = None;
        }
        if state.declined.as_deref() == Some(content) {
            return Admission::Suppressed;
        }
        if requester == Requester::Unattended {
            state.stage(content);
            return Admission::Queued;
        }
        let riding = state
            .in_flight
            .as_ref()
            .filter(|(running, _)| running == content)
            .map(|(_, covers)| *covers);
        let mine = match riding {
            Some(covers) => {
                if state.pending.is_none() {
                    state.pending = Some((content.to_owned(), covers));
                }
                covers
            },
            None => state.stage(content),
        };
        loop {
            if let Some(finished) = state.last.as_ref().filter(|f| f.covers >= mine) {
                return Admission::Joined(finished.outcome);
            }
            if state.in_flight.is_none() {
                break;
            }
            state = self
                .done
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        let (run, covers) = state
            .pending
            .take()
            .unwrap_or_else(|| (content.to_owned(), mine));
        state.in_flight = Some((run.clone(), covers));
        drop(state);
        Admission::Run(Ticket {
            gate: self,
            content: run,
            covers,
            finished: false,
        })
    }

    #[must_use]
    pub fn pending(&self) -> Option<String> {
        self.lock()
            .pending
            .as_ref()
            .map(|(content, _)| content.clone())
    }

    #[must_use]
    pub fn declined(&self) -> Option<String> {
        self.lock().declined.clone()
    }

    fn finish(&self, content: &str, covers: u64, outcome: GateOutcome) {
        let mut state = self.lock();
        state.in_flight = None;
        if outcome == GateOutcome::Declined {
            state.declined = Some(content.to_owned());
        }
        if outcome == GateOutcome::Applied
            && state.pending.as_ref().is_some_and(|(c, _)| c == content)
        {
            state.pending = None;
        }
        state.last = Some(Finished { covers, outcome });
        drop(state);
        self.done.notify_all();
    }
}

/// The right to run the one elevated write; it must be finished, and a
/// ticket dropped unfinished counts as a failed write.
#[derive(Debug)]
pub struct Ticket<'a> {
    gate: &'a ElevationGate,
    content: String,
    covers: u64,
    finished: bool,
}

impl Ticket<'_> {
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn finish(mut self, outcome: GateOutcome) {
        self.finished = true;
        self.gate.finish(&self.content, self.covers, outcome);
    }
}

impl Drop for Ticket<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.gate
                .finish(&self.content, self.covers, GateOutcome::Failed);
        }
    }
}

impl ElevationGate {
    pub fn note_pending(&self, content: &str) {
        let mut state = self.lock();
        if state.declined.as_deref() != Some(content) {
            state.stage(content);
        }
    }

    pub fn reset_decline(&self) {
        self.lock().declined = None;
    }
}
