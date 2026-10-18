//! What the host asks a plane to do: the apply direction, who asked, which
//! entities it may touch, and when a written change takes effect.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::UserId;

use super::DeclaredState;

/// The direction of one apply.
///
/// [`ApplyMode::SeedIfEmpty`] is the boot seed: the host issues it only
/// against an empty projection (and turns it into a no-op otherwise), so a
/// plane writes it exactly as [`ApplyMode::Overwrite`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyMode {
    InsertOnly,
    Overwrite,
    SeedIfEmpty,
}

impl ApplyMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InsertOnly => "insert_only",
            Self::Overwrite => "overwrite",
            Self::SeedIfEmpty => "seed_if_empty",
        }
    }

    #[must_use]
    pub const fn must_converge(self) -> bool {
        !matches!(self, Self::InsertOnly)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeActor {
    Boot,
    Seed,
    Reconciler,
    Cli,
    User(UserId),
}

impl ChangeActor {
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Boot => "boot",
            Self::Seed => "seed",
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

/// Which entities an apply may touch: the whole plane, or the named
/// [`super::PlaneSnapshot`] keys alone.
///
/// The host refuses a scoped apply to a plane whose
/// [`super::ConfigPlane::supports_scoped_apply`] is false, so a scoped caller
/// never reaches a plane-wide write.
#[derive(Debug, Clone, Copy, Default)]
pub enum EntityScope<'a> {
    #[default]
    All,
    Entities(&'a [String]),
}

impl EntityScope<'_> {
    #[must_use]
    pub fn admits(&self, entity: &str) -> bool {
        match self {
            Self::All => true,
            Self::Entities(keys) => keys.iter().any(|k| k == entity),
        }
    }

    #[must_use]
    pub const fn is_all(&self) -> bool {
        matches!(self, Self::All)
    }
}

/// One apply as the host hands it to [`super::ConfigPlane::apply`].
#[derive(Debug, Clone, Copy)]
pub struct ApplyRequest<'a> {
    pub declared: &'a DeclaredState,
    pub mode: ApplyMode,
    pub change: &'a ChangeRequest<'a>,
    pub scope: EntityScope<'a>,
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

/// When a change the plane writes reaches the running system: on the next
/// read (`live`), or only after the servers restart (`on_restart`), because
/// the runtime loads the projection once at boot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaneEffect {
    #[default]
    Live,
    OnRestart,
}

impl PlaneEffect {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::OnRestart => "on_restart",
        }
    }
}
