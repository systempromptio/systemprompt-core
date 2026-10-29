//! Jittered retry delays.
//!
//! A retry loop that sleeps a fixed or doubling delay keeps every replica, or
//! every client, that failed together retrying together, so an outage ends in
//! a synchronised reconnect storm. [`jittered`] spreads a delay uniformly over
//! its upper half: the floor keeps the backoff meaningful and the spread
//! breaks the lockstep. The randomness comes from the standard library's
//! per-hasher keys, so no RNG dependency reaches the shared layer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::hash::{BuildHasher, RandomState};
use std::time::Duration;

#[must_use]
pub fn jittered(delay: Duration) -> Duration {
    let millis = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX);
    let half = millis / 2;
    if half == 0 {
        return delay;
    }
    let draw = RandomState::new().hash_one(millis) % (half + 1);
    Duration::from_millis(millis - half + draw)
}
