//! Runs the golden scenarios (`crates/tests/golden/scenarios`) against the
//! harness, per `documentation/future/harness/testing/03-golden-scenarios.md`
//! section 4.
//!
//! For each scenario and protocol: a fresh scripted instance loaded with the
//! scenario's turns and verdicts, a [`relay`] in front of it that answers the
//! session-title call, a fresh data root signed in with
//! `systemprompt_harness::command::login`, the fixture copied to a fresh
//! workspace outside the data root with one modification time, and
//! `run_controlled` driven in process with the scenario's answers and
//! actions ([`drive`]). [`outcome`] normalises what the run left, compares
//! it with `expected/` and classifies every difference into a ledger row.

mod checks;
pub mod drive;
pub mod host;
pub mod outcome;
pub mod relay;
pub mod scenario;
mod watch;

pub use drive::{RunOutput, drive};
pub use outcome::{Outcome, judge};
pub use scenario::{Action, ActionOn, Scenario};
