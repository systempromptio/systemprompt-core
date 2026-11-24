//! The two read-side traits a route set's state and a provide fn implement
//! over a [`Resolved`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::MissingDependency;

use super::Resolved;
use crate::contribute::error::ProvideError;

/// A value resolved from the built extension state; written by hand for every
/// route set's state type.
pub trait FromState: Sized + Send + Sync + 'static {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency>;
}

/// A deferred provider of one state value, run at admission in composition
/// order.
pub trait ProvideFn<T>:
    Fn(&Resolved<'_>) -> Result<T, ProvideError> + Send + Sync + 'static
{
}

impl<T, F> ProvideFn<T> for F where
    F: Fn(&Resolved<'_>) -> Result<T, ProvideError> + Send + Sync + 'static
{
}
