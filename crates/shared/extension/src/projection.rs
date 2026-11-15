//! A projection a drainer job works off.
//!
//! A member binds a drain to the job type that drains it with
//! `Contributions::projection::<J>(drain)`; the scheduler hands every admitted
//! drain bound to `J` to that job.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::TypeId;
use std::time::Duration;

/// One projection a drainer job works off, with its batch and time budget.
#[derive(Debug, Clone, Copy)]
pub struct ProjectionDrain {
    pub projection: &'static str,
    pub projector: &'static str,
    pub batch: i32,
    pub budget: Duration,
}

/// A contributed drain and the job type it is bound to.
#[derive(Debug, Clone, Copy)]
pub struct ProjectionEntry {
    job: TypeId,
    drain: ProjectionDrain,
}

impl ProjectionEntry {
    pub(crate) fn of<J: 'static>(drain: ProjectionDrain) -> Self {
        Self {
            job: TypeId::of::<J>(),
            drain,
        }
    }

    #[must_use]
    pub fn is_for<J: 'static>(&self) -> bool {
        self.job == TypeId::of::<J>()
    }

    #[must_use]
    pub const fn job(&self) -> TypeId {
        self.job
    }

    #[must_use]
    pub const fn drain(&self) -> ProjectionDrain {
        self.drain
    }
}
