//! Start-time proof that no two lock holders share an advisory key.
//!
//! A job lock's key is `hashtext(name)::bigint`, so two distinct names whose
//! 32-bit `hashtext` agree would exclude each other and each would read
//! the other's tick as "held by another replica". [`find_lock_key_collision`]
//! checks the keys of every lock holder — each registered job plus the
//! config-reconcile lock — and the scheduler refuses to start on the first
//! collision. Holders are distinct by construction, so two entries with one
//! name (a job named like the reconcile lock) collide too.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::JobName;

/// Two lock holders that derive the same advisory key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockKeyCollision {
    pub first: JobName,
    pub second: JobName,
    pub key: i64,
}

#[must_use]
pub fn find_lock_key_collision(keys: &[(JobName, i64)]) -> Option<LockKeyCollision> {
    let mut seen: HashMap<i64, &JobName> = HashMap::with_capacity(keys.len());
    let mut sorted: Vec<&(JobName, i64)> = keys.iter().collect();
    sorted.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    for (name, key) in sorted {
        if let Some(first) = seen.get(key) {
            return Some(LockKeyCollision {
                first: (*first).clone(),
                second: name.clone(),
                key: *key,
            });
        }
        seen.insert(*key, name);
    }
    None
}
