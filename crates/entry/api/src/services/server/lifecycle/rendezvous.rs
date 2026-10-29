//! Rendezvous (highest-random-weight) assignment of process keys to workers.
//!
//! Every worker scores every `(key, worker)` pair with the same stable hash
//! and the highest score owns the key, so all workers agree on the owner from
//! the live worker set alone, and a worker joining or leaving moves only the
//! keys it wins or held. The hash is `FNV-1a` with a `SplitMix64` finaliser,
//! fixed here rather than taken from `std`, whose hasher may change between
//! toolchains and would split a mixed-version fleet's view of ownership.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::InstanceId;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a(state: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(state, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

const fn splitmix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

pub fn score(key: &str, worker: &InstanceId) -> u64 {
    let hash = fnv1a(FNV_OFFSET, key.as_bytes());
    let hash = fnv1a(hash, &[0xff]);
    splitmix64(fnv1a(hash, worker.as_str().as_bytes()))
}

pub fn preferred_owner<'a>(key: &str, workers: &'a [InstanceId]) -> Option<&'a InstanceId> {
    workers.iter().max_by(|a, b| {
        score(key, a)
            .cmp(&score(key, b))
            .then_with(|| a.as_str().cmp(b.as_str()))
    })
}
