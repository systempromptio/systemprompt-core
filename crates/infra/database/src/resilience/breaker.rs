//! A circuit breaker that fast-fails calls to an unhealthy dependency.

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use super::config::BreakerConfig;

/// Breaker mode. `Open` rejects calls until its cooldown elapses; `HalfOpen`
/// admits a limited number of probes to test recovery.
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

/// Returned by [`CircuitBreaker::acquire`] when the breaker rejects a call.
#[derive(Debug, Clone, Copy)]
pub struct Tripped;

/// A `Closed` → `Open` → `HalfOpen` circuit breaker keyed to one dependency.
///
/// [`CircuitBreaker::acquire`] is called before a guarded operation;
/// [`CircuitBreaker::record_success`] / [`CircuitBreaker::record_failure`]
/// report its outcome. The breaker also accepts out-of-band failure reports
/// (e.g. from a health monitor) via the same `record_*` methods.
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

    /// Admit a call, or reject it with [`Tripped`] if the breaker is open.
    ///
    /// An `Open` breaker past its cooldown transitions to `HalfOpen` and admits
    /// the caller as a probe.
    pub fn acquire(&self) -> Result<(), Tripped> {
        let mut state = self.lock();
        let result = match state.mode {
            Mode::Closed => Ok(()),
            Mode::Open => {
                let cooled_down = state
                    .open_until
                    .is_some_and(|until| Instant::now() >= until);
                if cooled_down {
                    self.transition(&mut state, Mode::HalfOpen);
                    state.probes_in_flight = 1;
                    Ok(())
                } else {
                    Err(Tripped)
                }
            },
            Mode::HalfOpen => {
                if state.probes_in_flight < self.cfg.half_open_max_probes {
                    state.probes_in_flight += 1;
                    Ok(())
                } else {
                    Err(Tripped)
                }
            },
        };
        drop(state);
        result
    }

    /// Report a successful call. Closes the breaker if it was probing.
    pub fn record_success(&self) {
        let mut state = self.lock();
        state.consecutive_failures = 0;
        state.probes_in_flight = state.probes_in_flight.saturating_sub(1);
        if state.mode != Mode::Closed {
            self.transition(&mut state, Mode::Closed);
            state.open_until = None;
        }
    }

    /// Report a failed call. Opens the breaker once the failure threshold is
    /// reached, or immediately if a half-open probe failed.
    pub fn record_failure(&self) {
        let mut state = self.lock();
        state.probes_in_flight = state.probes_in_flight.saturating_sub(1);
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);

        let should_open = state.mode == Mode::HalfOpen
            || state.consecutive_failures >= self.cfg.failure_threshold;
        if should_open && state.mode != Mode::Open {
            self.transition(&mut state, Mode::Open);
            state.open_until = Some(Instant::now() + self.cfg.open_cooldown);
        }
    }

    /// Whether the breaker is currently rejecting calls.
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
