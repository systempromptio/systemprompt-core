//! Invalidation epochs for in-memory caches whose source of truth is a table
//! any replica may write.
//!
//! One [`CacheEpochs`] handle is built at the composition root and cloned into
//! the relay and every cache. A cache records [`CacheEpochs::current`] for its
//! [`CacheDomain`] when it fills an entry and treats the entry as stale once
//! the epoch has moved. The epoch is
//! advanced by the cross-replica `LISTEN` relay when a write on any replica
//! notifies the domain, so a change becomes visible everywhere within one
//! notification round trip; each cache's own TTL stays the bound when a
//! notification is lost. The relay also advances every domain when it
//! (re)connects, since notifications sent while it was down are gone.
//!
//! A config plane's domain is notified as `config:<plane>` by the
//! `sp_bump_config_version()` trigger; [`CacheDomain::from_notification`]
//! reads both that form and a bare domain label. A long-lived watcher (a
//! [`crate::ConfigWatch`]) waits on [`CacheEpochs::subscribe`] instead of
//! polling the counter.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use tokio::sync::watch;

const CONFIG_PREFIX: &str = "config:";
const DOMAINS: usize = 8;

/// A family of cached rows invalidated together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheDomain {
    GatewayPolicy,
    Authz,
    JtiRevocation,
    GatewayRoutes,
    Governance,
    Groups,
    AccessControl,
    GatewayPolicies,
}

impl CacheDomain {
    pub const ALL: [Self; DOMAINS] = [
        Self::GatewayPolicy,
        Self::Authz,
        Self::JtiRevocation,
        Self::GatewayRoutes,
        Self::Governance,
        Self::Groups,
        Self::AccessControl,
        Self::GatewayPolicies,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::GatewayPolicy => "gateway_policy",
            Self::Authz => "authz",
            Self::JtiRevocation => "jti_revocation",
            Self::GatewayRoutes => "gateway_routes",
            Self::Governance => "governance",
            Self::Groups => "groups",
            Self::AccessControl => "access_control",
            Self::GatewayPolicies => "gateway_policies",
        }
    }

    #[must_use]
    pub const fn config_plane(self) -> Option<&'static str> {
        match self {
            Self::GatewayPolicy | Self::Authz | Self::JtiRevocation => None,
            Self::GatewayRoutes
            | Self::Governance
            | Self::Groups
            | Self::AccessControl
            | Self::GatewayPolicies => Some(self.label()),
        }
    }

    #[must_use]
    pub fn parse(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|domain| domain.label() == label)
    }

    #[must_use]
    pub fn from_notification(payload: &str) -> Option<Self> {
        payload.strip_prefix(CONFIG_PREFIX).map_or_else(
            || Self::parse(payload),
            |plane| {
                Self::ALL
                    .into_iter()
                    .find(|domain| domain.config_plane() == Some(plane))
            },
        )
    }

    const fn slot(self) -> usize {
        match self {
            Self::GatewayPolicy => 0,
            Self::Authz => 1,
            Self::JtiRevocation => 2,
            Self::GatewayRoutes => 3,
            Self::Governance => 4,
            Self::Groups => 5,
            Self::AccessControl => 6,
            Self::GatewayPolicies => 7,
        }
    }
}

/// Shared invalidation epochs, one per [`CacheDomain`], plus whether a relay
/// is currently listening; clones share state.
///
/// A handle starts not live. Only the relay marks it live, once its `LISTEN`
/// is established, and clears it when the connection drops or the relay
/// stops. While not live a cache cannot learn of a write on another replica,
/// so a security-relevant cache (the JTI "not revoked" answer) is not trusted
/// and others shorten their bound.
#[derive(Debug, Clone, Default)]
pub struct CacheEpochs {
    inner: Arc<EpochSlots>,
}

#[derive(Debug)]
struct EpochSlots {
    slots: [AtomicU64; DOMAINS],
    signals: [watch::Sender<u64>; DOMAINS],
    relay_live: AtomicBool,
}

impl Default for EpochSlots {
    fn default() -> Self {
        Self {
            slots: Default::default(),
            signals: std::array::from_fn(|_| watch::Sender::new(0)),
            relay_live: AtomicBool::new(false),
        }
    }
}

impl CacheEpochs {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn current(&self, domain: CacheDomain) -> u64 {
        self.inner.slots[domain.slot()].load(Ordering::Acquire)
    }

    pub fn advance(&self, domain: CacheDomain) -> u64 {
        let epoch = self.inner.slots[domain.slot()].fetch_add(1, Ordering::AcqRel) + 1;
        self.inner.signals[domain.slot()].send_replace(epoch);
        epoch
    }

    #[must_use]
    pub fn subscribe(&self, domain: CacheDomain) -> watch::Receiver<u64> {
        self.inner.signals[domain.slot()].subscribe()
    }

    #[must_use]
    pub fn relay_live(&self) -> bool {
        self.inner.relay_live.load(Ordering::Acquire)
    }

    pub fn set_relay_live(&self, live: bool) {
        self.inner.relay_live.store(live, Ordering::Release);
    }

    pub fn advance_all(&self) {
        for domain in CacheDomain::ALL {
            self.advance(domain);
        }
    }
}
