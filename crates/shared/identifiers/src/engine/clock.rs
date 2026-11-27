//! The clock behind minted ids.
//!
//! A minted id is `<prefix>_<16 hex: milliseconds << 12 | counter><10 hex:
//! random>`. The time field is fixed width and strictly increasing within one
//! [`IdClock`] (it never goes backwards, even when the wall clock does), so
//! comparing two ids of one clock as strings compares their creation order.
//! The random tail keeps ids minted from different clocks apart.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::error::IdError;

const COUNTER_BITS: u32 = 12;
const TIME_DIGITS: usize = 16;
const SUFFIX_DIGITS: usize = 10;
const SUFFIX_MASK: u64 = (1u64 << 40) - 1;

/// The source of minted ids. Ids that are ordered against each other are
/// minted from the same clock: the owner of the records builds one and hands
/// it to whatever mints for it.
#[derive(Debug, Default)]
pub struct IdClock {
    last_stamp: AtomicU64,
}

impl IdClock {
    pub const fn new() -> Self {
        Self {
            last_stamp: AtomicU64::new(0),
        }
    }

    fn next_stamp(&self) -> u64 {
        let floor = u64::try_from(now_ms()).unwrap_or(0) << COUNTER_BITS;
        let mut previous = self.last_stamp.load(Ordering::Relaxed);
        loop {
            let candidate = floor.max(previous.saturating_add(1));
            match self.last_stamp.compare_exchange_weak(
                previous,
                candidate,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return candidate,
                Err(actual) => previous = actual,
            }
        }
    }

    pub(crate) fn mint(&self, prefix: &str) -> String {
        let stamp = self.next_stamp();
        let suffix = RandomState::new().hash_one(stamp) & SUFFIX_MASK;
        format!("{prefix}_{stamp:016x}{suffix:010x}")
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

pub(crate) fn validate(prefix: &str, kind: &'static str, value: &str) -> Result<(), IdError> {
    let valid = value
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('_'))
        .is_some_and(|body| {
            body.len() == TIME_DIGITS + SUFFIX_DIGITS
                && body
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        });
    if valid {
        Ok(())
    } else {
        Err(IdError {
            kind,
            value: value.to_owned(),
        })
    }
}

pub(crate) fn created_ms(value: &str) -> i64 {
    value
        .split_once('_')
        .and_then(|(_, body)| body.get(..TIME_DIGITS))
        .and_then(|hex| u64::from_str_radix(hex, 16).ok())
        .map_or(0, |stamp| {
            i64::try_from(stamp >> COUNTER_BITS).unwrap_or(i64::MAX)
        })
}
