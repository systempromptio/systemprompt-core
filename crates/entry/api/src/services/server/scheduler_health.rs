//! Record of jobs the scheduler dropped at startup, owned by the server.
//!
//! [`SchedulerHealth::record`] is called after
//! [`systemprompt_scheduler::SchedulerService::start`] returns, with the jobs
//! whose explicit owner did not resolve. [`handle_health`] reads
//! [`SchedulerHealth::degraded`] to report the scheduler as `degraded` so a
//! partially disabled scheduler surfaces in monitoring instead of failing
//! silently.
//!
//! [`handle_health`]: super::health::handle_health
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{PoisonError, RwLock};

use systemprompt_scheduler::SkippedJob;

/// The scheduler's startup degradation for one API server.
///
/// Built once in `bind_and_serve` and handed to the health route as an axum
/// extension; a second server in the same process (tests) has its own.
#[derive(Debug, Default)]
pub struct SchedulerHealth {
    degraded: RwLock<Vec<SkippedJob>>,
}

impl SchedulerHealth {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, jobs: Vec<SkippedJob>) {
        *self
            .degraded
            .write()
            .unwrap_or_else(PoisonError::into_inner) = jobs;
    }

    #[must_use]
    pub fn degraded(&self) -> Vec<SkippedJob> {
        self.degraded
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}
