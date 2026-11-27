//! Which poll of an account runs next, with which options, and the wait after
//! it.
//!
//! Requests merge: one asked for while another runs is held and runs once after
//! it, carrying every option asked for meanwhile. Only Sync now asks the
//! instance for a fresh build; Sync now, a regained focus and the loop's start
//! read the profile and model catalog again, which otherwise are read every
//! five minutes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use sp_protocol::view::SyncErrorKind;

use crate::sync::cadence::{CATALOG_INTERVAL, Jitter, poll_interval, service_delay, transient_delay};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Start,
    Timer,
    Focus,
    SyncNow,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Request {
    pub no_cache: bool,
    pub reread: bool,
}

impl Request {
    const fn of(trigger: Trigger) -> Self {
        match trigger {
            Trigger::Start | Trigger::Focus => Self {
                no_cache: false,
                reread: true,
            },
            Trigger::SyncNow => Self {
                no_cache: true,
                reread: true,
            },
            Trigger::Timer => Self {
                no_cache: false,
                reread: false,
            },
        }
    }

    const fn merge(self, other: Self) -> Self {
        Self {
            no_cache: self.no_cache || other.no_cache,
            reread: self.reread || other.reread,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Synced,
    Failed {
        kind: SyncErrorKind,
        retry_after: Option<Duration>,
    },
    Halted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    Wait(Duration),
    Halt,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Planner {
    pending: Option<Request>,
    failures: u32,
    catalog_read_at: Option<Instant>,
}

impl Planner {
    pub fn new() -> Self {
        let mut planner = Self::default();
        planner.request(Trigger::Start);
        planner
    }

    pub fn request(&mut self, trigger: Trigger) {
        let asked = Request::of(trigger);
        self.pending = Some(self.pending.map_or(asked, |held| held.merge(asked)));
    }

    pub const fn take(&mut self) -> Option<Request> {
        self.pending.take()
    }

    pub fn catalog_due(&self, request: Request, now: Instant) -> bool {
        request.reread
            || self
                .catalog_read_at
                .is_none_or(|read| now.saturating_duration_since(read) >= CATALOG_INTERVAL)
    }

    pub const fn catalog_read(&mut self, now: Instant) {
        self.catalog_read_at = Some(now);
    }

    pub fn finished(&mut self, outcome: Outcome, focused: bool, jitter: &mut dyn Jitter) -> Next {
        let interval = poll_interval(focused, jitter);
        match outcome {
            Outcome::Synced => {
                self.failures = 0;
                Next::Wait(interval)
            },
            Outcome::Halted => {
                self.failures = 0;
                Next::Halt
            },
            Outcome::Failed { kind, retry_after } => {
                self.failures = self.failures.saturating_add(1);
                Next::Wait(match kind {
                    SyncErrorKind::Unreachable | SyncErrorKind::RateLimited => {
                        transient_delay(self.failures, retry_after)
                    },
                    SyncErrorKind::Service => service_delay(self.failures, interval),
                    SyncErrorKind::Refused
                    | SyncErrorKind::KeyChanged
                    | SyncErrorKind::Invalid
                    | SyncErrorKind::Insecure => interval,
                })
            },
        }
    }
}
