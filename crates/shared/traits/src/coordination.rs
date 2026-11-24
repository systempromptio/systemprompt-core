//! Cross-replica coordination seams: replica liveness, time-bounded resource
//! leases and session-scoped advisory locks.
//!
//! Core owns the `services` heartbeat, the boot-time instance claim and the
//! session pool (the pool that never sits behind a transaction-mode pooler).
//! These traits expose that machinery so an extension never re-derives
//! liveness from raw `services` SQL with its own staleness window, or takes
//! an advisory lock on a pool that may hand its backend to another client.
//!
//! The traits are consumed as `Arc<dyn …>` through the extension context, so
//! they rely on `#[async_trait]` until native `async fn` is `dyn`-compatible.
//! [`UnavailableCoordination`] is the refusing implementation a context
//! without a database hands out.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{AdvisoryLockKey, InstanceId, LeaseKey};

use crate::repository::RepositoryError;

#[derive(Debug, thiserror::Error)]
pub enum CoordinationError {
    #[error("{seam} is not available in this context")]
    Unavailable { seam: &'static str },

    #[error("lease ttl must be between one second and one day, got {ttl:?}")]
    InvalidTtl { ttl: Duration },

    #[error("lease {key} is no longer held under fencing token {token}")]
    Fenced { key: LeaseKey, token: FencingToken },

    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

pub type CoordinationResult<T> = Result<T, CoordinationError>;

/// Liveness of a replica, judged by its boot-time instance claim and the
/// `services` heartbeat core maintains.
///
/// Held as `Arc<dyn InstanceRegistry>` (`DynInstanceRegistry`), hence
/// `#[async_trait]`.
#[async_trait]
pub trait InstanceRegistry: Send + Sync {
    async fn is_alive(&self, instance: &InstanceId) -> CoordinationResult<bool>;
}

/// The monotonic token a lease claim is stamped with.
///
/// Every claim that changes a lease's owner draws a strictly larger token
/// than any token the key carried before, so a holder that lost its lease
/// holds a token smaller than its replacement's; a write fenced on the token
/// ([`LeaseRepository::renew`]) is refused once a newer one exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FencingToken(i64);

impl FencingToken {
    #[must_use]
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}

impl std::fmt::Display for FencingToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    pub key: LeaseKey,
    pub owner: InstanceId,
    pub fencing_token: FencingToken,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Exclusive, expiring ownership of a named resource by one replica.
///
/// `claim` takes the lease when it is free, expired, held by a replica that
/// is no longer alive, or already ours — in which case it extends the
/// expiry and keeps the token, so a holder heartbeats by claiming again
/// before `ttl` elapses. A claim that changes the owner stamps a new, larger
/// [`FencingToken`]. `None` means another live replica holds it.
///
/// `renew` is the fenced write: it extends the lease only while the row
/// still carries this holder's token, and otherwise changes nothing and
/// returns [`CoordinationError::Fenced`]. `release` is fenced the same
/// way: it deletes the row only while it still carries this holder's token,
/// so a holder that lost its lease never releases the new holder's.
///
/// Held as `Arc<dyn LeaseRepository>` (`DynLeaseRepository`), hence
/// `#[async_trait]`.
#[async_trait]
pub trait LeaseRepository: Send + Sync {
    async fn claim(&self, key: &LeaseKey, ttl: Duration) -> CoordinationResult<Option<Lease>>;

    async fn renew(&self, lease: &Lease, ttl: Duration) -> CoordinationResult<Lease>;

    async fn release(&self, lease: &Lease) -> CoordinationResult<()>;

    async fn holder(&self, key: &LeaseKey) -> CoordinationResult<Option<Lease>>;
}

/// A held session-scoped advisory lock. `release` unlocks on the session
/// that locked; dropping without releasing closes that session, which frees
/// the lock server-side.
///
/// Boxed as `Box<dyn HeldAdvisoryLock>` (`AdvisoryLockGuard`), hence
/// `#[async_trait]`.
#[async_trait]
pub trait HeldAdvisoryLock: Send + Sync + std::fmt::Debug {
    fn key(&self) -> &AdvisoryLockKey;

    async fn release(self: Box<Self>);
}

pub type AdvisoryLockGuard = Box<dyn HeldAdvisoryLock>;

/// Cluster-wide advisory locks taken on the session pool, in a key space
/// disjoint from the scheduler's job locks and the instance claims.
///
/// Held as `Arc<dyn AdvisoryLocks>`, hence `#[async_trait]`.
#[async_trait]
pub trait AdvisoryLocks: Send + Sync {
    async fn try_acquire(
        &self,
        key: &AdvisoryLockKey,
    ) -> CoordinationResult<Option<AdvisoryLockGuard>>;

    async fn acquire_within(
        &self,
        key: &AdvisoryLockKey,
        wait: Duration,
    ) -> CoordinationResult<Option<AdvisoryLockGuard>>;
}

pub type DynInstanceRegistry = Arc<dyn InstanceRegistry>;
pub type DynLeaseRepository = Arc<dyn LeaseRepository>;
pub type DynAdvisoryLocks = Arc<dyn AdvisoryLocks>;

#[derive(Debug, Clone, Copy, Default)]
pub struct UnavailableCoordination;

#[async_trait]
impl InstanceRegistry for UnavailableCoordination {
    async fn is_alive(&self, _instance: &InstanceId) -> CoordinationResult<bool> {
        Err(CoordinationError::Unavailable {
            seam: "instance registry",
        })
    }
}

#[async_trait]
impl LeaseRepository for UnavailableCoordination {
    async fn claim(&self, _key: &LeaseKey, _ttl: Duration) -> CoordinationResult<Option<Lease>> {
        Err(CoordinationError::Unavailable { seam: "leases" })
    }

    async fn renew(&self, _lease: &Lease, _ttl: Duration) -> CoordinationResult<Lease> {
        Err(CoordinationError::Unavailable { seam: "leases" })
    }

    async fn release(&self, _lease: &Lease) -> CoordinationResult<()> {
        Err(CoordinationError::Unavailable { seam: "leases" })
    }

    async fn holder(&self, _key: &LeaseKey) -> CoordinationResult<Option<Lease>> {
        Err(CoordinationError::Unavailable { seam: "leases" })
    }
}

#[async_trait]
impl AdvisoryLocks for UnavailableCoordination {
    async fn try_acquire(
        &self,
        _key: &AdvisoryLockKey,
    ) -> CoordinationResult<Option<AdvisoryLockGuard>> {
        Err(CoordinationError::Unavailable {
            seam: "advisory locks",
        })
    }

    async fn acquire_within(
        &self,
        _key: &AdvisoryLockKey,
        _wait: Duration,
    ) -> CoordinationResult<Option<AdvisoryLockGuard>> {
        Err(CoordinationError::Unavailable {
            seam: "advisory locks",
        })
    }
}
