//! [`DoctorCheck`]: a preflight check a member contributes to `cloud doctor`.
//!
//! `cloud doctor` runs before any boot: it holds the checked profile's
//! secrets and the composition that profile admits, but no kernel state. A
//! member that owns a deployment prerequisite contributes a [`DoctorCheck`]
//! value with `Contributions::doctor_check`; the doctor runs every admitted
//! member's checks, in composition order, and reports each by its
//! [`DoctorCheck::name`].
//!
//! A check reads only what its [`DoctorContext`] hands it: whether a named
//! secret is present (never its value, through [`SecretPresence`]) and the
//! host handles the doctor opened for the run (a database pool when the
//! profile's database is reachable) in a [`Dependencies`] map. A check that
//! cannot run because a handle is absent reports [`DoctorStatus::Warn`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::hash::BuildHasher;

use async_trait::async_trait;

use crate::dependencies::Dependencies;

/// The verdict of one doctor check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorStatus {
    Pass,
    Warn,
    Fail,
}

/// One doctor check's verdict and the line the doctor prints for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorOutcome {
    pub status: DoctorStatus,
    pub detail: String,
}

impl DoctorOutcome {
    #[must_use]
    pub fn pass(detail: impl Into<String>) -> Self {
        Self {
            status: DoctorStatus::Pass,
            detail: detail.into(),
        }
    }

    #[must_use]
    pub fn warn(detail: impl Into<String>) -> Self {
        Self {
            status: DoctorStatus::Warn,
            detail: detail.into(),
        }
    }

    #[must_use]
    pub fn fail(detail: impl Into<String>) -> Self {
        Self {
            status: DoctorStatus::Fail,
            detail: detail.into(),
        }
    }
}

/// Whether the checked profile's secrets hold a non-empty value under a name.
pub trait SecretPresence: Send + Sync {
    fn is_present(&self, name: &str) -> bool;
}

impl<S: BuildHasher + Send + Sync> SecretPresence for HashMap<String, String, S> {
    fn is_present(&self, name: &str) -> bool {
        self.get(name).is_some_and(|value| !value.is_empty())
    }
}

/// What `cloud doctor` hands a contributed check.
#[derive(Clone, Copy)]
pub struct DoctorContext<'a> {
    pub secrets: &'a dyn SecretPresence,
    pub handles: &'a Dependencies,
}

impl std::fmt::Debug for DoctorContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DoctorContext")
            .field("handles", self.handles)
            .finish_non_exhaustive()
    }
}

/// A deployment prerequisite a member checks for `cloud doctor`.
///
/// Held as `Arc<dyn DoctorCheck>` in the admitted composition, so
/// `#[async_trait]` is required for `dyn` dispatch.
#[async_trait]
pub trait DoctorCheck: Send + Sync {
    fn name(&self) -> &'static str;

    async fn check(&self, ctx: DoctorContext<'_>) -> DoctorOutcome;
}
