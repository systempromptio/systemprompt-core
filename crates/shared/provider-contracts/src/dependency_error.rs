//! Why a [`Dependencies`](crate::Dependencies) read or provide failed: an
//! absent (or withheld) type ([`MissingDependency`]), or a second provider of
//! one type ([`DuplicateDependency`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, type_name};

use systemprompt_identifiers::ExtensionId;
use thiserror::Error;

use crate::dependencies::DependencyProvider;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("provider context has no {type_name} dependency")]
pub struct MissingDependency {
    type_name: &'static str,
}

impl MissingDependency {
    #[must_use]
    pub fn of<T: Any>() -> Self {
        Self {
            type_name: type_name::<T>(),
        }
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        self.type_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "{type_name} is already provided by {first}; extension '{second}' cannot provide it again, \
     every type has exactly one provider"
)]
pub struct DuplicateDependency {
    pub type_name: &'static str,
    pub first: DependencyProvider,
    pub second: ExtensionId,
}
