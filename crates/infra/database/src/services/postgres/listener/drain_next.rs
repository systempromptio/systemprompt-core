//! [`DrainNext`]: what a durable outbox consumer does after a drain round.
//!
//! A round that worked drains again at once; one that found nothing waits on
//! its [`super::PollWake`]; one cut short by transient failures, or one that
//! failed outright, backs off for the poll interval and ignores wakes, so a
//! database outage is not retried at the pace of the listener reconnecting.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// The consumer's next step after a drain round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainNext {
    Drain,
    Listen,
    BackOff,
}

impl DrainNext {
    #[must_use]
    pub const fn after(worked: bool, cut_short: bool) -> Self {
        if cut_short {
            Self::BackOff
        } else if worked {
            Self::Drain
        } else {
            Self::Listen
        }
    }

    #[must_use]
    pub const fn of<E>(round: &Result<Self, E>) -> Self {
        match round {
            Ok(next) => *next,
            Err(_) => Self::BackOff,
        }
    }
}
