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
//! lowercase `snake_case` token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod diff;
mod error;

use std::collections::BTreeMap;
use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use systemprompt_identifiers::UserId;

use crate::dependencies::{Dependencies, MissingDependency};

pub use diff::{DriftKind, DriftRow, diff_snapshots};
pub use error::{ConfigPlaneError, ConfigPlaneResult};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyMode {
    InsertOnly,
    Overwrite,
}

impl ApplyMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InsertOnly => "insert_only",
            Self::Overwrite => "overwrite",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeActor {
    Boot,
    Reconciler,
    Cli,
    User(UserId),
}

impl ChangeActor {
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Boot => "boot",
            Self::Reconciler => "config_reconcile",
            Self::Cli => "cli",
            Self::User(id) => id.as_str(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChangeRequest<'a> {
    pub actor: ChangeActor,
    pub reason: &'a str,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ApplyOutcome {
    pub inserted: usize,
    pub updated: usize,
    pub deleted: usize,
}

impl ApplyOutcome {
    #[must_use]
    pub const fn total(&self) -> usize {
        self.inserted + self.updated + self.deleted
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
/// `apply` with [`ApplyMode::Overwrite`] must leave the database equal to the
/// declaration (a following [`ConfigPlane::read_current`] diffs clean): the
/// host re-reads after every correction and reports a plane whose apply did
/// not converge as failed rather than corrected. The host inserts `DbPool`
/// into the context's dependencies. Planes are held as `Arc<dyn ConfigPlane>`,
/// and native `async fn` in traits is not `dyn`-compatible.
#[async_trait]
pub trait ConfigPlane: Send + Sync {
    fn name(&self) -> &'static str;

    fn declared_path(&self) -> &'static str;

    fn load_declared(&self, services_root: &Path) -> ConfigPlaneResult<Option<DeclaredState>>;

    async fn read_current(&self, ctx: &PlaneContext<'_>) -> ConfigPlaneResult<PlaneSnapshot>;

    fn diff(&self, declared: &DeclaredState, current: &PlaneSnapshot) -> Vec<DriftRow> {
        diff_snapshots(&declared.entries, current)
    }

    async fn apply(
        &self,
        ctx: &PlaneContext<'_>,
        declared: &DeclaredState,
        mode: ApplyMode,
        change: &ChangeRequest<'_>,
    ) -> ConfigPlaneResult<ApplyOutcome>;
}
