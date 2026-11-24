//! Member route surfaces the API mounts under its own layering.
//!
//! A surface is one builder a core member provides open: the API mount calls
//! it with the member's route-scoped resolution and keeps the policy, context,
//! rate limit and role group of the routes it returns. The member owns the
//! handlers; the API owns the layering around them. A surface whose member is
//! not admitted is not found, and its routes are not mounted (404).
//!
//! - [`SyncSurface`] (`cloud`): the services-tree manifest and download
//!   routes under `/api/v1/sync`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;

use crate::{MissingDependency, Resolved};

/// Builds the services-tree sync routes from the `cloud` route state.
pub trait SyncSurface: Send + Sync + std::fmt::Debug {
    fn build(&self, resolved: &Resolved<'_>) -> Result<Router, MissingDependency>;
}

pub type DynSyncSurface = Arc<dyn SyncSurface>;
