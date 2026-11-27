//! Poll intervals, jitter and every backoff of the sync engine, pure over an
//! injected clock and jitter source.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use chrono::{DateTime, Utc};

pub const FOCUSED_INTERVAL: Duration = Duration::from_secs(30);
pub const JITTER: f64 = 0.2;
pub const UNFOCUSED_INTERVAL: Duration = Duration::from_secs(300);
pub const BACKOFF_FIRST: Duration = Duration::from_secs(30);
pub const BACKOFF_CAP: Duration = Duration::from_secs(300);
pub const CATALOG_INTERVAL: Duration = Duration::from_secs(300);
pub const SERVICE_RETRY: Duration = Duration::from_secs(2);
pub const ENROL_RETRY: Duration = Duration::from_secs(2);
pub const RETRY_CAP: Duration = Duration::from_secs(3600);
pub const INSTALL_BUDGET: Duration = Duration::from_secs(30);
pub const INSTALL_MINIMUM: Duration = Duration::from_secs(2);
pub const DELIVERY_BUDGET: Duration = Duration::from_secs(10);

const OUTBOX_EXPONENT_MAX: u32 = 12;

/// The wall clock the engine stamps statuses and schedules retries with;
/// injected so a fake can drive it.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// A source of uniform values in `[0, 1]` that spreads the focused poll
/// interval.
pub trait Jitter: Send {
    fn unit(&mut self) -> f64;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RandomJitter;

impl Jitter for RandomJitter {
    fn unit(&mut self) -> f64 {
        rand::random::<f64>()
    }
}

pub fn poll_interval(focused: bool, jitter: &mut dyn Jitter) -> Duration {
    if !focused {
        return UNFOCUSED_INTERVAL;
    }
    let unit = jitter.unit();
    let unit = if unit.is_finite() {
        unit.clamp(0.0, 1.0)
    } else {
        0.5
    };
    FOCUSED_INTERVAL.mul_f64(JITTER.mul_add(2.0f64.mul_add(unit, -1.0), 1.0))
}

pub fn doubling(first: Duration, attempt: u32, cap: Duration) -> Duration {
    let exponent = attempt.saturating_sub(1).min(31);
    first.saturating_mul(1u32 << exponent).min(cap)
}

pub fn transient_delay(failures: u32, retry_after: Option<Duration>) -> Duration {
    doubling(BACKOFF_FIRST, failures, BACKOFF_CAP)
        .max(retry_after.unwrap_or(Duration::ZERO))
        .min(BACKOFF_CAP)
}

pub fn service_delay(failures: u32, interval: Duration) -> Duration {
    doubling(SERVICE_RETRY, failures, interval)
}

pub fn enrol_delay(attempts: u32) -> Duration {
    doubling(ENROL_RETRY, attempts, RETRY_CAP)
}

pub fn outbox_delay(attempts: u32) -> Duration {
    Duration::from_secs(1u64 << attempts.min(OUTBOX_EXPONENT_MAX)).min(RETRY_CAP)
}

pub fn after(now: DateTime<Utc>, delay: Duration) -> DateTime<Utc> {
    chrono::TimeDelta::from_std(delay)
        .ok()
        .and_then(|delta| now.checked_add_signed(delta))
        .unwrap_or(DateTime::<Utc>::MAX_UTC)
}
