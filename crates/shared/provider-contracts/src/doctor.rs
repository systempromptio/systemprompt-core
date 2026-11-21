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
//! read-only, purpose-typed facts of [`DoctorProbes`]. It holds no database
//! pool, connection or handle map; a fact a check needs is a probe method the
//! doctor answers with a fixed read in a read-only session. A probe whose
//! source is unreachable answers [`DoctorProbeError::Unreachable`], and the
//! check reports [`DoctorStatus::Warn`].
//!
//! Every admitted member's checks run, core and install alike, against the
//! one context the doctor builds for the run: the view is the same for every
//! standing, so an install check obtains nothing a core check does not, and
//! nothing in that view reads a secret value or writes the tenant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::hash::BuildHasher;

use async_trait::async_trait;

use systemprompt_identifiers::ManagedSourceId;

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

/// A registered Git source that names a credential label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCredentialSource {
    pub id: ManagedSourceId,
    pub name: String,
    pub repository: String,
    pub credential_reference: String,
}

/// Why a doctor probe could not answer.
#[derive(Debug, thiserror::Error)]
pub enum DoctorProbeError {
    #[error("the checked profile's database is not reachable")]
    Unreachable,
    #[error("the probe read failed")]
    Read(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// The read-only facts `cloud doctor` answers for a contributed check.
///
/// Each method is one fixed read the doctor runs in a read-only session;
/// none takes SQL, a table name or a secret name. Held as `&dyn`, so
/// `#[async_trait]` is required for `dyn` dispatch.
#[async_trait]
pub trait DoctorProbes: Send + Sync {
    async fn git_credential_sources(&self) -> Result<Vec<GitCredentialSource>, DoctorProbeError>;
}

/// What `cloud doctor` hands a contributed check: secret presence and
/// read-only probes, the same for every member's standing.
#[derive(Clone, Copy)]
pub struct DoctorContext<'a> {
    pub secrets: &'a dyn SecretPresence,
    pub probes: &'a dyn DoctorProbes,
}

impl std::fmt::Debug for DoctorContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DoctorContext").finish_non_exhaustive()
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
