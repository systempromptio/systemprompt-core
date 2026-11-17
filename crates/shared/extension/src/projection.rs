//! A projection a drainer job works off.
//!
//! A member binds a drain with `Contributions::projection::<J>(drain)`, where
//! `J` is a [`DrainTarget`]: the member's own job type, or a kernel drain
//! marker in [`kernel`] for a drainer job the kernel owns. The kernel collects
//! every admitted member's drains into [`AdmittedProjections`] and a drainer
//! job reads the entries bound to its target; a disabled member's drains are
//! never admitted, so they are never drained.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;
use std::time::Duration;

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::Job;

use crate::admitted::BuiltState;

/// One projection a drainer job works off, with its batch and time budget.
#[derive(Debug, Clone, Copy)]
pub struct ProjectionDrain {
    pub projection: &'static str,
    pub projector: &'static str,
    pub batch: i32,
    pub budget: Duration,
}

/// What a drain binds to: a member's own [`Job`] type, or a kernel drain
/// marker in [`kernel`].
pub trait DrainTarget: 'static {}

impl<J: Job + 'static> DrainTarget for J {}

/// Drain markers for the drainer jobs the kernel owns, so a member binds a
/// drain to one without naming the crate that runs it.
pub mod kernel {
    use super::DrainTarget;

    /// The scheduler's `backfill_rollups` job, which rebuilds the queued hours
    /// of the hourly usage rollups.
    #[derive(Debug, Clone, Copy)]
    pub struct BackfillRollups;

    impl DrainTarget for BackfillRollups {}
}

/// A contributed drain and the target it is bound to.
#[derive(Debug, Clone, Copy)]
pub struct ProjectionEntry {
    target: TypeId,
    drain: ProjectionDrain,
}

impl ProjectionEntry {
    pub(crate) const fn of<J: DrainTarget>(drain: ProjectionDrain) -> Self {
        Self {
            target: TypeId::of::<J>(),
            drain,
        }
    }

    #[must_use]
    pub fn is_for<J: DrainTarget>(&self) -> bool {
        self.target == TypeId::of::<J>()
    }

    #[must_use]
    pub const fn target(&self) -> TypeId {
        self.target
    }

    #[must_use]
    pub const fn drain(&self) -> ProjectionDrain {
        self.drain
    }
}

/// Every admitted member's drains with their owners, in composition order.
#[derive(Debug, Clone, Default)]
pub struct AdmittedProjections {
    entries: Vec<(ExtensionId, ProjectionEntry)>,
}

impl AdmittedProjections {
    #[must_use]
    pub fn of(built: &BuiltState) -> Self {
        let entries = built
            .members()
            .flat_map(|member| {
                member
                    .sink
                    .projections()
                    .iter()
                    .map(move |entry| (member.owner.clone(), *entry))
            })
            .collect();
        Self { entries }
    }

    pub fn drains_for<J: DrainTarget>(
        &self,
    ) -> impl Iterator<Item = (&ExtensionId, ProjectionDrain)> + '_ {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.is_for::<J>())
            .map(|(owner, entry)| (owner, entry.drain()))
    }
}
