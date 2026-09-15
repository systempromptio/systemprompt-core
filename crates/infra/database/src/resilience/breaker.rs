//! A circuit breaker that fast-fails calls to an unhealthy dependency.
//!
//! Admission is a [`Probe`] token: while it lives it occupies one of the
//! half-open probe slots, and it must be settled with `success`/`failure`.
//! A probe dropped unsettled — a cancelled future — frees its slot without
//! changing the breaker's mode, so a client disconnect can never exhaust the
//! probe budget and leave the breaker open forever.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use super::config::BreakerConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug)]
struct State {
    mode: Mode,
    consecutive_failures: u32,
    open_until: Option<Instant>,
    probes_in_flight: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Tripped;

/// One admitted call. Settle it with [`Probe::success`] or
/// [`Probe::failure`]; dropping it unsettled releases the probe slot only.
#[derive(Debug)]
#[must_use = "an unsettled probe neither closes nor reopens the breaker"]
pub struct Probe<'a> {
    breaker: &'a CircuitBreaker,
    counted: bool,
    settled: bool,
}

impl Probe<'_> {
    pub fn success(mut self) {
        self.settled = true;
        self.breaker.settle(self.counted, true);
    }

    pub fn failure(mut self) {
        self.settled = true;
        self.breaker.settle(self.counted, false);
    }
}

impl Drop for Probe<'_> {
    fn drop(&mut self) {
        if !self.settled && self.counted {
            let mut state = self.breaker.lock();
            state.probes_in_flight = state.probes_in_flight.saturating_sub(1);
        }
    }
}

#[derive(Debug)]
pub struct CircuitBreaker {
    key: String,
    cfg: BreakerConfig,
    state: Mutex<State>,
}

impl CircuitBreaker {
    pub fn new(key: impl Into<String>, cfg: BreakerConfig) -> Self {
        Self {
            key: key.into(),
            cfg,
            state: Mutex::new(State {
                mode: Mode::Closed,
                consecutive_failures: 0,
                open_until: None,
                probes_in_flight: 0,
            }),
        }
    }

    pub fn acquire(&self) -> Result<Probe<'_>, Tripped> {
        let mut state = self.lock();
        let counted = match state.mode {
            Mode::Closed => false,
            Mode::Open => {
                let cooled_down = state
                    .open_until
                    .is_some_and(|until| Instant::now() >= until);
                if !cooled_down {
                    return Err(Tripped);
                }
                self.transition(&mut state, Mode::HalfOpen);
                state.probes_in_flight = 1;
                true
            },
            Mode::HalfOpen => {
                if state.probes_in_flight >= self.cfg.half_open_max_probes {
                    return Err(Tripped);
                }
                state.probes_in_flight += 1;
                true
            },
        };
        drop(state);
        Ok(Probe {
            breaker: self,
            counted,
            settled: false,
        })
    }

    pub fn record_success(&self) {
        self.settle(false, true);
    }

    pub fn record_failure(&self) {
        self.settle(false, false);
    }

    fn settle(&self, counted: bool, success: bool) {
        let mut state = self.lock();
        if counted {
            state.probes_in_flight = state.probes_in_flight.saturating_sub(1);
        }
        if success {
            state.consecutive_failures = 0;
            if state.mode != Mode::Closed {
                self.transition(&mut state, Mode::Closed);
                state.open_until = None;
            }
            return;
        }
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);

        let should_open = state.mode == Mode::HalfOpen
            || state.consecutive_failures >= self.cfg.failure_threshold;
        if should_open && state.mode != Mode::Open {
            self.transition(&mut state, Mode::Open);
            state.open_until = Some(Instant::now() + self.cfg.open_cooldown);
        }
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.lock().mode == Mode::Open
    }

    fn transition(&self, state: &mut State, to: Mode) {
        let from = state.mode;
        if from != to {
            state.mode = to;
            tracing::warn!(key = %self.key, ?from, ?to, "circuit breaker state transition");
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
