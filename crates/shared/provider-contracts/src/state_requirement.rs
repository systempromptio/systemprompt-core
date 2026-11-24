//! [`StateRequirement`]: a type a member, job or runner service cannot run
//! without. [`StateRequirement::is_met_by`] checks it against a built
//! [`Dependencies`] map, so the host refuses boot instead of failing a run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, TypeId, type_name};

use crate::dependencies::Dependencies;

/// A type a member, job or runner service declares it cannot run without,
/// checked against the built state before anything serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateRequirement {
    type_id: TypeId,
    type_name: &'static str,
}

impl StateRequirement {
    #[must_use]
    pub fn of<T: Any + Send + Sync>() -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            type_name: type_name::<T>(),
        }
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }

    #[must_use]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    #[must_use]
    pub fn is_met_by(&self, state: &Dependencies) -> bool {
        state.contains_type(self.type_id)
    }
}
