//! The fleet-level fold over per-agent verdicts: the summary card and the
//! headline are computed from verdicts alone, so the card and the rows are
//! structurally incapable of disagreeing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

use super::agent_health::{AgentState, AgentVerdict};

/// How the fleet as a whole reads.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FleetState {
    Ok,
    Warn,
    Err,
    #[default]
    Unknown,
}

/// The one-line summary code for the fleet — also the FTL key suffix.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FleetHeadline {
    AllWorking,
    NeedsAttention,
    NotWorking,
    Checking,
    #[default]
    NoneEnabled,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct AgentFleetSummary {
    pub total: usize,
    pub working: usize,
    pub ready: usize,
    pub attention: usize,
    pub not_set_up: usize,
    pub down: usize,
    pub checking: usize,
    pub running: usize,
    pub state: FleetState,
    pub headline: FleetHeadline,
}

impl AgentFleetSummary {
    // Why: takes only verdicts — never the raw snapshot — so the card and the rows
    // are structurally incapable of reaching different conclusions.
    #[must_use]
    pub fn fold<'a>(verdicts: impl Iterator<Item = &'a AgentVerdict>) -> Self {
        let mut s = Self {
            state: FleetState::Unknown,
            headline: FleetHeadline::NoneEnabled,
            ..Self::default()
        };
        for v in verdicts {
            s.total += 1;
            if v.is_running {
                s.running += 1;
            }
            match v.state {
                AgentState::Working => s.working += 1,
                AgentState::Ready => s.ready += 1,
                AgentState::Attention => s.attention += 1,
                AgentState::NotSetUp => s.not_set_up += 1,
                AgentState::Down => s.down += 1,
                AgentState::Checking => s.checking += 1,
            }
        }

        s.state = if s.total == 0 {
            FleetState::Unknown
        } else if s.down > 0 {
            FleetState::Err
        } else if s.attention > 0 || s.not_set_up > 0 {
            FleetState::Warn
        } else if s.checking == s.total {
            FleetState::Unknown
        } else {
            FleetState::Ok
        };

        s.headline = match s.state {
            FleetState::Ok => FleetHeadline::AllWorking,
            FleetState::Warn => FleetHeadline::NeedsAttention,
            FleetState::Err => FleetHeadline::NotWorking,
            FleetState::Unknown if s.total == 0 => FleetHeadline::NoneEnabled,
            FleetState::Unknown => FleetHeadline::Checking,
        };
        s
    }
}

/// Both scopes the GUI needs: every enabled agent, and only those set up here.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct AgentFleets {
    pub all: AgentFleetSummary,
    pub set_up: AgentFleetSummary,
}

impl AgentFleets {
    #[must_use]
    pub fn fold(verdicts: &[AgentVerdict]) -> Self {
        Self {
            all: AgentFleetSummary::fold(verdicts.iter()),
            set_up: AgentFleetSummary::fold(verdicts.iter().filter(|v| v.is_set_up)),
        }
    }
}
