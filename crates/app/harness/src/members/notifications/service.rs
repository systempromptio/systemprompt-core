//! The feed service: polls the followed instance's feed, backs off on
//! failure, and waits for the next account while withheld.
//!
//! The composition root holds it as `dyn BackgroundService`, which is why its
//! `run` is an `async_trait` method.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use crate::extension::{BackgroundService, CancellationToken};
use async_trait::async_trait;

use crate::members::notifications::error::NotificationsError;
use crate::members::notifications::feed::catch_up;
use crate::members::notifications::handle::{NotificationsHandle, Target};
use crate::members::notifications::inbox::FeedStatus;

const MAX_DOUBLINGS: u32 = 16;

/// How often the feed is polled and how a failing poll backs off.
///
/// A successful poll waits `interval`; the n-th failure in a row waits
/// `retry_floor` doubled n-1 times, at most `retry_ceiling`, or the
/// instance's `Retry-After` when it sent one; an instance with no feed is
/// asked again after `not_served_recheck`. A refresh request or a change of
/// account cuts any wait short.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollCadence {
    pub interval: Duration,
    pub retry_floor: Duration,
    pub retry_ceiling: Duration,
    pub not_served_recheck: Duration,
}

impl Default for PollCadence {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            retry_floor: Duration::from_secs(5),
            retry_ceiling: Duration::from_mins(5),
            not_served_recheck: Duration::from_mins(15),
        }
    }
}

impl PollCadence {
    fn backoff(&self, failures: u32) -> Duration {
        let doublings = failures.saturating_sub(1).min(MAX_DOUBLINGS);
        self.retry_floor
            .saturating_mul(1 << doublings)
            .min(self.retry_ceiling)
    }
}

/// Keeps a [`NotificationsHandle`]'s inbox current for as long as it runs.
#[derive(Debug, Clone)]
pub struct NotificationsService {
    handle: NotificationsHandle,
    cadence: PollCadence,
}

impl NotificationsService {
    pub const fn new(handle: NotificationsHandle, cadence: PollCadence) -> Self {
        Self { handle, cadence }
    }

    async fn poll(&self, target: &Target, failures: &mut u32) -> Duration {
        match catch_up(&self.handle, target).await {
            Ok(()) => {
                *failures = 0;
                self.cadence.interval
            },
            Err(NotificationsError::NotServed { endpoint }) => {
                tracing::info!(endpoint, "the instance serves no notifications feed");
                *failures = 0;
                self.handle.update(target.generation, |inbox| {
                    inbox.set_status(FeedStatus::NotServed);
                });
                self.cadence.not_served_recheck
            },
            Err(error) => {
                *failures = failures.saturating_add(1);
                let retry_in = error
                    .retry_after()
                    .unwrap_or_else(|| self.cadence.backoff(*failures));
                let kind = error.kind();
                tracing::warn!(
                    error = %error,
                    failures = *failures,
                    retry_in_ms = u64::try_from(retry_in.as_millis()).unwrap_or(u64::MAX),
                    "notifications feed poll failed"
                );
                self.handle.update(target.generation, |inbox| {
                    inbox.set_status(FeedStatus::Failing { kind, retry_in });
                });
                retry_in
            },
        }
    }
}

#[async_trait]
impl BackgroundService for NotificationsService {
    fn name(&self) -> &'static str {
        "notifications-feed"
    }

    async fn run(&self, cancel: CancellationToken) {
        let mut targets = self.handle.targets();
        let mut failures = 0u32;
        loop {
            let target = targets.borrow_and_update().clone();
            let Some(target) = target else {
                failures = 0;
                tokio::select! {
                    () = cancel.cancelled() => return,
                    changed = targets.changed() => if changed.is_err() { return },
                }
                continue;
            };
            let wait = tokio::select! {
                () = cancel.cancelled() => return,
                wait = self.poll(&target, &mut failures) => wait,
            };
            tokio::select! {
                () = cancel.cancelled() => return,
                changed = targets.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    failures = 0;
                },
                () = self.handle.refresh_requested() => {},
                () = tokio::time::sleep(wait) => {},
            }
        }
    }
}
