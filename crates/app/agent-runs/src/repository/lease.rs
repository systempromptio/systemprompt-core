//! [`Lease`], the proof of a successful claim, and [`Fenced`], the outcome
//! of a write made under it.
//!
//! A lease is built only by
//! [`crate::repository::AgentRunRepository::claim_queued`] from the row it just
//! claimed: the claimer, the attempt and the claim time. Every write to a
//! claimed run takes the lease and matches all three in its predicate, so a
//! worker whose run was reclaimed (attempt bumped, claimer cleared) or released
//! and claimed again (new claim time) changes nothing
//! and gets [`Fenced::LeaseLost`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::task_link::RunClaim;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::AgentRunId;

use super::AgentRunRow;
use crate::error::{AgentRunsError, AgentRunsResult};

/// Ownership of one claim of one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    claimed_by: String,
    attempt: i32,
    claimed_at: DateTime<Utc>,
}

impl Lease {
    pub(super) const fn new(claimed_by: String, attempt: i32, claimed_at: DateTime<Utc>) -> Self {
        Self {
            claimed_by,
            attempt,
            claimed_at,
        }
    }

    #[must_use]
    pub fn claimed_by(&self) -> &str {
        &self.claimed_by
    }

    #[must_use]
    pub const fn attempt(&self) -> i32 {
        self.attempt
    }

    #[must_use]
    pub const fn claimed_at(&self) -> DateTime<Utc> {
        self.claimed_at
    }

    #[must_use]
    pub fn run_claim(&self) -> RunClaim<'_> {
        RunClaim::Lease {
            claimed_by: &self.claimed_by,
            attempt: self.attempt,
            claimed_at: self.claimed_at,
        }
    }
}

/// Whether a write under a [`Lease`] took effect.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fenced {
    Applied,
    LeaseLost,
}

impl Fenced {
    pub(super) const fn from_rows(rows: u64) -> Self {
        if rows == 0 {
            Self::LeaseLost
        } else {
            Self::Applied
        }
    }

    pub const fn is_applied(self) -> bool {
        matches!(self, Self::Applied)
    }

    pub fn applied(self, run_id: &AgentRunId) -> AgentRunsResult<()> {
        match self {
            Self::Applied => Ok(()),
            Self::LeaseLost => Err(AgentRunsError::LeaseLost {
                run_id: run_id.clone(),
            }),
        }
    }
}

/// What [`crate::repository::AgentRunRepository::release`] did with a run
/// its claimer stopped without finishing.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Released {
    Requeued,
    Exhausted,
    LeaseLost,
}

impl Released {
    pub fn applied(self, run_id: &AgentRunId) -> AgentRunsResult<Self> {
        match self {
            Self::LeaseLost => Err(AgentRunsError::LeaseLost {
                run_id: run_id.clone(),
            }),
            released => Ok(released),
        }
    }
}

/// A run taken by [`crate::repository::AgentRunRepository::claim_queued`] and
/// the lease that owns it.
#[derive(Debug, Clone)]
pub struct Claimed {
    pub row: AgentRunRow,
    pub lease: Lease,
}
