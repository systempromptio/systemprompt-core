//! Single-flight elevated writes: one at a time, never overlapping a profile
//! install, with a caller of the write already in flight taking its result.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::Any;
use std::sync::{Arc, PoisonError};

use super::{ElevationGate, GateOutcome};

#[derive(Clone)]
pub(super) struct FinishedWrite {
    content: String,
    run: u64,
    outcome: GateOutcome,
    value: Option<Arc<dyn Any + Send + Sync>>,
}

impl std::fmt::Debug for FinishedWrite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FinishedWrite")
            .field("content", &self.content)
            .field("run", &self.run)
            .field("outcome", &self.outcome)
            .finish_non_exhaustive()
    }
}

impl ElevationGate {
    pub fn single_flight<T, E>(
        &self,
        content: &str,
        run: impl FnOnce() -> Result<T, E>,
        is_declined: impl Fn(&E) -> bool,
        declined: impl FnOnce() -> E,
    ) -> Result<T, E>
    where
        T: Clone + Send + Sync + 'static,
    {
        let mut state = self.lock();
        // Why: a profile install that itself needs an elevated write (the
        // Windows machine policy) already holds the gate on this thread; that
        // write is part of the run in flight, not a second prompt to queue.
        if state.in_flight_owner == Some(std::thread::current().id()) {
            drop(state);
            return run();
        }
        loop {
            let joining = state
                .write_in_flight
                .as_ref()
                .filter(|(running, _)| running == content)
                .map(|(_, run)| *run);
            if let Some(target) = joining {
                while state.last_write.as_ref().is_none_or(|w| w.run < target) {
                    state = self
                        .done
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                let shared = state
                    .last_write
                    .as_ref()
                    .filter(|w| w.run == target && w.content == content)
                    .cloned();
                match shared {
                    Some(FinishedWrite {
                        outcome: GateOutcome::Applied,
                        value: Some(value),
                        ..
                    }) => {
                        if let Some(value) = value.downcast_ref::<T>() {
                            return Ok(value.clone());
                        }
                    },
                    Some(FinishedWrite {
                        outcome: GateOutcome::Declined,
                        ..
                    }) => return Err(declined()),
                    _ => {},
                }
                continue;
            }
            if state.in_flight.is_none() && state.write_in_flight.is_none() {
                break;
            }
            state = self
                .done
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        state.writes += 1;
        let run_id = state.writes;
        state.write_in_flight = Some((content.to_owned(), run_id));
        drop(state);
        let mut flight = WriteFlight {
            gate: self,
            content,
            run: run_id,
            finished: false,
        };
        let result = run();
        match &result {
            Ok(value) => flight.finish(GateOutcome::Applied, Some(Arc::new(value.clone()))),
            Err(error) if is_declined(error) => flight.finish(GateOutcome::Declined, None),
            Err(_) => flight.finish(GateOutcome::Failed, None),
        }
        result
    }

    fn finish_write(
        &self,
        content: &str,
        run: u64,
        outcome: GateOutcome,
        value: Option<Arc<dyn Any + Send + Sync>>,
    ) {
        let mut state = self.lock();
        state.write_in_flight = None;
        state.last_write = Some(FinishedWrite {
            content: content.to_owned(),
            run,
            outcome,
            value,
        });
        drop(state);
        self.done.notify_all();
    }
}

struct WriteFlight<'a> {
    gate: &'a ElevationGate,
    content: &'a str,
    run: u64,
    finished: bool,
}

impl WriteFlight<'_> {
    fn finish(&mut self, outcome: GateOutcome, value: Option<Arc<dyn Any + Send + Sync>>) {
        self.finished = true;
        self.gate
            .finish_write(self.content, self.run, outcome, value);
    }
}

impl Drop for WriteFlight<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.gate
                .finish_write(self.content, self.run, GateOutcome::Failed, None);
        }
    }
}
