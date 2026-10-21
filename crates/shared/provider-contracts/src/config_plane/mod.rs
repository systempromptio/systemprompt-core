//! [`ConfigPlane`] contract: one declared configuration file projected into
//! database rows, reconciled by the host.
//!
//! A plane owns one declaration under the services tree and the rows it
//! projects into. The host never learns either shape: a plane reads its
//! declaration into a [`PlaneSnapshot`] (entity key → canonical value), reads
//! the database into the same shape, and applies a direction when asked. The
//! host computes the drift ([`diff_snapshots`], overridable through
//! [`ConfigPlane::diff`]), decides from the plane's authority whether to
//! correct it or only report it, and records every correction with its
//! before and after values.
//!
//! [`ConfigPlane::load_declared`] is synchronous and reads only the services
//! tree, so `admin config validate` runs it offline. It returns `Ok(None)` when
//! the declaration file is absent, which the host treats as database
//! authority whatever the profile says: an absent file never empties a table.
//!
//! [`ConfigPlane::name`] is the profile key under `config_planes.authority`,
//! the metrics label and the audit row's plane column, so it is a stable
//! lowercase `snake_case` token. [`ConfigPlane::depends_on`] names the planes
//! whose rows this one references; [`ConfigPlaneSet`] orders the set so they
//! are reconciled first.
//!
//! [`ConfigPlane::export`] renders the current database state as declaration
//! text, which is how an operator captures console-only rows before switching
//! a plane to `file` authority.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod diff;
mod error;
mod request;
mod set;
mod tx;

use std::collections::BTreeMap;
use std::path::Path;

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use crate::dependencies::{Dependencies, MissingDependency};

pub use diff::{DriftKind, DriftRow, diff_snapshots};
pub use error::{ConfigPlaneError, ConfigPlaneResult};
pub use request::{
    ApplyMode, ApplyOutcome, ApplyRequest, ChangeActor, ChangeRequest, EntityScope, PlaneEffect,
};
pub use set::{ConfigPlaneSet, EffectiveAuthority, PlaneAuthority, PlaneWriter};
pub use tx::PlaneTx;

pub type PlaneSnapshot = BTreeMap<String, String>;

/// A plane's declaration as read from the services tree.
///
/// `hash` is a SHA-256 over the canonical entries, so two files that differ
/// only in formatting or ordering hash the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredState {
    pub hash: String,
    pub entries: PlaneSnapshot,
}

impl DeclaredState {
    #[must_use]
    pub fn from_entries(entries: PlaneSnapshot) -> Self {
        let mut digest = Sha256::new();
        for (key, value) in &entries {
            digest.update((key.len() as u64).to_be_bytes());
            digest.update(key.as_bytes());
            digest.update((value.len() as u64).to_be_bytes());
            digest.update(value.as_bytes());
        }
        Self {
            hash: hex::encode(digest.finalize()),
            entries,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PlaneContext<'a> {
    services_root: &'a Path,
    dependencies: &'a Dependencies,
}

impl<'a> PlaneContext<'a> {
    #[must_use]
    pub const fn new(services_root: &'a Path, dependencies: &'a Dependencies) -> Self {
        Self {
            services_root,
            dependencies,
        }
    }

    #[must_use]
    pub const fn services_root(&self) -> &'a Path {
        self.services_root
    }

    pub fn get<T: std::any::Any + Send + Sync>(&self) -> Result<&'a T, MissingDependency> {
        self.dependencies.get::<T>()
    }
}

/// One declared configuration plane an extension contributes through
/// `Extension::config_planes`.
///
/// Every database read and write runs on the host's transaction
/// ([`PlaneTx`]); the host records the audit rows in it and commits once.
/// `apply` with [`ApplyMode::Overwrite`] or [`ApplyMode::SeedIfEmpty`] must
/// leave the admitted entities equal to the declaration (a following
/// [`ConfigPlane::read_current`] on the same transaction diffs clean): the
/// host re-reads after every apply and rolls back an apply that did not
/// converge. The host inserts `DbPool` into the context's dependencies.
///
/// The defaulted methods refuse: [`ConfigPlane::export`] answers
/// [`ConfigPlaneError::Unsupported`], and a scoped apply reaches a plane only
/// when [`ConfigPlane::supports_scoped_apply`] is true. Planes are held as
/// `Arc<dyn ConfigPlane>`, and native `async fn` in traits is not
/// `dyn`-compatible.
#[async_trait]
pub trait ConfigPlane: Send + Sync {
    fn name(&self) -> &'static str;

    fn declared_path(&self) -> &'static str;

    fn depends_on(&self) -> &'static [&'static str] {
        &[]
    }

    fn effect(&self) -> PlaneEffect {
        PlaneEffect::Live
    }

    fn supports_scoped_apply(&self) -> bool {
        false
    }

    fn load_declared(&self, services_root: &Path) -> ConfigPlaneResult<Option<DeclaredState>>;

    async fn read_current(
        &self,
        ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<PlaneSnapshot>;

    fn diff(&self, declared: &DeclaredState, current: &PlaneSnapshot) -> Vec<DriftRow> {
        diff_snapshots(&declared.entries, current)
    }

    async fn apply(
        &self,
        ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
        request: &ApplyRequest<'_>,
    ) -> ConfigPlaneResult<ApplyOutcome>;

    async fn export(
        &self,
        _ctx: &PlaneContext<'_>,
        _tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<Option<String>> {
        Err(ConfigPlaneError::Unsupported {
            plane: self.name().to_owned(),
            operation: "export",
        })
    }
}
