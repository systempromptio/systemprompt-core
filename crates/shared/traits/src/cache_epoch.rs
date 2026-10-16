//! Process-wide invalidation epochs for in-memory caches whose source of truth
//! is a table any replica may write.
//!
//! A cache records [`cache_epoch`] for its [`CacheDomain`] when it fills an
//! entry and treats the entry as stale once the epoch has moved. The epoch is
//! advanced by the cross-replica `LISTEN` relay when a write on any replica
//! notifies the domain, so a change becomes visible everywhere within one
//! notification round trip; each cache's own TTL stays the bound when a
//! notification is lost. The relay also advances every domain when it
//! (re)connects, since notifications sent while it was down are gone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::{AtomicU64, Ordering};

/// A family of cached rows invalidated together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheDomain {
    GatewayPolicy,
    Authz,
    JtiRevocation,
}

impl CacheDomain {
    pub const ALL: [Self; 3] = [Self::GatewayPolicy, Self::Authz, Self::JtiRevocation];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::GatewayPolicy => "gateway_policy",
            Self::Authz => "authz",
            Self::JtiRevocation => "jti_revocation",
        }
    }

    #[must_use]
    pub fn parse(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|domain| domain.label() == label)
    }

    const fn slot(self) -> usize {
        match self {
            Self::GatewayPolicy => 0,
            Self::Authz => 1,
            Self::JtiRevocation => 2,
        }
    }
}

static EPOCHS: [AtomicU64; 3] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];

#[must_use]
pub fn cache_epoch(domain: CacheDomain) -> u64 {
    EPOCHS[domain.slot()].load(Ordering::Acquire)
}

pub fn advance_cache_epoch(domain: CacheDomain) -> u64 {
    EPOCHS[domain.slot()].fetch_add(1, Ordering::AcqRel) + 1
}
