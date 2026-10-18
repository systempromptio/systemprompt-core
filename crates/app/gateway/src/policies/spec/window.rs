//! Quota window boundaries.
//!
//! A window is either a fixed span of `window_seconds` aligned to the Unix
//! epoch, or a UTC calendar period (`day`, `week` starting Monday, `month`).
//! A calendar window's bucket is keyed by its period start and its period
//! length in seconds, so every request in the same calendar month lands in
//! one bucket and nothing has to be carried across days.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Datelike, Duration, Months, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};

/// A UTC calendar period a quota window resets on.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CalendarPeriod {
    Day,
    Week,
    Month,
}

/// The bucket a window resolves to at an instant: `[start, end)`, keyed by
/// `start` and `bucket_seconds`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSpan {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub bucket_seconds: i32,
}

impl WindowSpan {
    #[must_use]
    pub fn fixed(now: DateTime<Utc>, window_seconds: i32) -> Self {
        let w = i64::from(window_seconds.max(1));
        let aligned = now.timestamp().div_euclid(w) * w;
        let start = Utc.timestamp_opt(aligned, 0).single().unwrap_or(now);
        Self {
            start,
            end: start + Duration::seconds(w),
            bucket_seconds: window_seconds.max(1),
        }
    }

    #[must_use]
    pub fn calendar(now: DateTime<Utc>, period: CalendarPeriod) -> Self {
        let today = now.date_naive();
        let (first, next) = match period {
            CalendarPeriod::Day => (today, today + Duration::days(1)),
            CalendarPeriod::Week => {
                let first =
                    today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
                (first, first + Duration::days(7))
            },
            CalendarPeriod::Month => {
                let first =
                    NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
                (first, first + Months::new(1))
            },
        };
        let start = Utc.from_utc_datetime(&first.and_time(chrono::NaiveTime::MIN));
        let end = Utc.from_utc_datetime(&next.and_time(chrono::NaiveTime::MIN));
        let bucket_seconds = i32::try_from((end - start).num_seconds()).unwrap_or(i32::MAX);
        Self {
            start,
            end,
            bucket_seconds,
        }
    }
}
