//! [`PlaneTx`]: the host's database transaction, handed to a plane without
//! this crate naming the driver.
//!
//! Every plane read and write the host drives runs inside one transaction the
//! host opened, and the host writes the `config_change_audit` rows for an
//! apply into that same transaction before it commits. A plane recovers the
//! concrete handle with [`PlaneTx::get`]; the platform's host inserts
//! `sqlx::Transaction<'static, sqlx::Postgres>`. A plane that writes outside
//! it loses the atomicity of change and audit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::{Any, type_name};
use std::fmt;

use crate::dependencies::MissingDependency;

/// A borrowed, type-erased transaction for the duration of one plane call.
pub struct PlaneTx<'a> {
    inner: &'a mut (dyn Any + Send),
    type_name: &'static str,
}

impl<'a> PlaneTx<'a> {
    pub fn new<T: Any + Send>(inner: &'a mut T) -> Self {
        Self {
            inner,
            type_name: type_name::<T>(),
        }
    }

    pub fn get<T: Any + Send>(&mut self) -> Result<&mut T, MissingDependency> {
        self.inner
            .downcast_mut::<T>()
            .ok_or_else(MissingDependency::of::<T>)
    }
}

impl fmt::Debug for PlaneTx<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlaneTx")
            .field("type_name", &self.type_name)
            .finish()
    }
}
