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

    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

pub type CoordinationResult<T> = Result<T, CoordinationError>;

/// Liveness of a replica, judged by its boot-time instance claim and the
/// `services` heartbeat core maintains.
#[async_trait]
pub trait InstanceRegistry: Send + Sync {
    async fn is_alive(&self, instance: &InstanceId) -> CoordinationResult<bool>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    pub key: LeaseKey,
    pub owner: InstanceId,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Exclusive, expiring ownership of a named resource by one replica.
///
/// `claim` takes the lease when it is free, expired, held by a replica that
/// is no longer alive, or already ours — in which case it extends the
/// expiry, so a holder heartbeats by claiming again before `ttl` elapses.
/// `None` means another live replica holds it.
#[async_trait]
pub trait LeaseRepository: Send + Sync {
    async fn claim(&self, key: &LeaseKey, ttl: Duration) -> CoordinationResult<Option<Lease>>;

    async fn release(&self, key: &LeaseKey) -> CoordinationResult<bool>;

    async fn holder(&self, key: &LeaseKey) -> CoordinationResult<Option<Lease>>;
}

/// A held session-scoped advisory lock. `release` unlocks on the session
/// that locked; dropping without releasing closes that session, which frees
/// the lock server-side.
#[async_trait]
pub trait HeldAdvisoryLock: Send + Sync + std::fmt::Debug {
    fn key(&self) -> &AdvisoryLockKey;

    async fn release(self: Box<Self>);
}

pub type AdvisoryLockGuard = Box<dyn HeldAdvisoryLock>;

/// Cluster-wide advisory locks taken on the session pool, in a key space
/// disjoint from the scheduler's job locks and the instance claims.
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

    async fn release(&self, _key: &LeaseKey) -> CoordinationResult<bool> {
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
