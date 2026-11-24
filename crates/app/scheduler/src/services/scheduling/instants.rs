//! The scheduler's clock and the cron instants a tick belongs to.
//!
//! A tick is keyed by the cron instant it fires for, computed from the
//! replica's clock as the latest instant of the schedule at or before
//! [`SchedulerClock::now`] taken to the whole second, since cron instants
//! are whole seconds: a replica whose clock runs ahead or behind fires
//! when its own clock reaches the instant and computes the same instant, so
//! every replica claims the same run row for it. The clock is a constructor
//! collaborator of the scheduler ([`super::SchedulerService::with_clock`]);
//! [`SystemClock`] is the wall clock.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, SubsecRound, Utc};
use croner::Cron;
use croner::parser::{CronParser, Seconds};

use crate::error::{SchedulerError, SchedulerResult};

/// The time source the scheduler keys ticks and missed instants by.
pub trait SchedulerClock: Send + Sync + std::fmt::Debug {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl SchedulerClock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

pub type DynSchedulerClock = Arc<dyn SchedulerClock>;

/// A job's cron schedule, parsed the way the cron loop parses it.
#[derive(Debug, Clone)]
pub struct CronInstants {
    cron: Cron,
}

impl CronInstants {
    pub fn parse(schedule: &str) -> SchedulerResult<Self> {
        CronParser::builder()
            .seconds(Seconds::Required)
            .dom_and_dow(true)
            .build()
            .parse(schedule)
            .map(|cron| Self { cron })
            .map_err(|source| SchedulerError::InvalidCron {
                schedule: schedule.to_owned(),
                source,
            })
    }

    pub fn at_or_before(&self, now: DateTime<Utc>) -> SchedulerResult<DateTime<Utc>> {
        self.cron
            .find_previous_occurrence(&now.trunc_subsecs(0), true)
            .map_err(|source| SchedulerError::InvalidCron {
                schedule: self.cron.pattern.to_string(),
                source,
            })
    }

    #[must_use]
    pub fn missed(
        &self,
        after: DateTime<Utc>,
        now: DateTime<Utc>,
        limit: usize,
    ) -> Vec<DateTime<Utc>> {
        let Ok(latest) = self.at_or_before(now) else {
            return Vec::new();
        };
        let mut missed: Vec<DateTime<Utc>> = std::iter::once(latest)
            .chain(self.cron.iter_before(latest))
            .take_while(|instant| *instant > after)
            .take(limit)
            .collect();
        missed.reverse();
        missed
    }
}
