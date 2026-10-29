//! One administrator approval at a time, for the newest content.
//!
//! Every machine-policy change that needs elevation passes this gate. At most
//! one elevated write runs per process; a request that arrives while one is in
//! flight waits for it and, when it asked for the same content, takes its
//! result instead of prompting again. Requests for different content coalesce:
//! the next run applies whatever was staged last. A declined prompt suppresses
//! further prompts for that content until the user asks again or the content
//! changes, and an unattended caller never prompts — it records the content as
//! pending for the GUI to offer.
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
    in_flight: Option<String>,
    pending: Option<String>,
    declined: Option<String>,
    last: Option<(String, GateOutcome)>,
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
            state.pending = Some(content.to_owned());
            return Admission::Queued;
        }
        state.pending = Some(content.to_owned());
        loop {
            if state.in_flight.is_none() {
                break;
            }
            let waited_for = state.in_flight.clone();
            state = self
                .done
                .wait_while(state, |s| {
                    s.in_flight.is_some() && s.in_flight == waited_for
                })
                .unwrap_or_else(PoisonError::into_inner);
            if let Some((ran, outcome)) = state.last.clone()
                && (ran == content || state.pending.as_deref() != Some(content))
            {
                return Admission::Joined(outcome);
            }
        }
        let run = state.pending.take().unwrap_or_else(|| content.to_owned());
        state.in_flight = Some(run.clone());
        drop(state);
        Admission::Run(Ticket {
            gate: self,
            content: run,
            finished: false,
        })
    }

    #[must_use]
    pub fn pending(&self) -> Option<String> {
        self.lock().pending.clone()
    }

    #[must_use]
    pub fn declined(&self) -> Option<String> {
        self.lock().declined.clone()
    }

    fn finish(&self, content: &str, outcome: GateOutcome) {
        let mut state = self.lock();
        state.in_flight = None;
        if outcome == GateOutcome::Declined {
            state.declined = Some(content.to_owned());
        }
        if outcome == GateOutcome::Applied && state.pending.as_deref() == Some(content) {
            state.pending = None;
        }
        state.last = Some((content.to_owned(), outcome));
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
    finished: bool,
}

impl Ticket<'_> {
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn finish(mut self, outcome: GateOutcome) {
        self.finished = true;
        self.gate.finish(&self.content, outcome);
    }
}

impl Drop for Ticket<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.gate.finish(&self.content, GateOutcome::Failed);
        }
    }
}

impl ElevationGate {
    pub fn note_pending(&self, content: &str) {
        let mut state = self.lock();
        if state.declined.as_deref() != Some(content) {
            state.pending = Some(content.to_owned());
        }
    }

    pub fn reset_decline(&self) {
        self.lock().declined = None;
    }
}
