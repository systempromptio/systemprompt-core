//! The marketplace state the kernel-mounted managed and gateway routes read:
//! the managed repository, the catalogue cache and the per-user filter of the
//! built marketplace seams, each provided by the `managed_resources` member
//! into the kernel state.
//!
//! The member is required, so an absent value is a composition fault; it is
//! reported as a managed-resource internal failure, the 5xx of every caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::MissingDependency;
use systemprompt_marketplace::contract::MarketplaceSeams;
use systemprompt_marketplace::managed::{ManagedError, ManagedRepository};
use systemprompt_marketplace::{MarketplaceCache, MarketplaceFilter};
use systemprompt_runtime::AppContext;

fn unprovided(error: MissingDependency) -> ManagedError {
    ManagedError::Internal {
        context: "managed resources state",
        source: Box::new(error),
    }
}

pub(crate) fn managed_repository(
    ctx: &AppContext,
) -> Result<&Arc<ManagedRepository>, ManagedError> {
    ctx.kernel()
        .state()
        .get::<Arc<ManagedRepository>>()
        .map_err(unprovided)
}

pub(crate) fn marketplace_cache(ctx: &AppContext) -> Result<&Arc<MarketplaceCache>, ManagedError> {
    ctx.kernel()
        .state()
        .get::<Arc<MarketplaceCache>>()
        .map_err(unprovided)
}

pub(crate) fn marketplace_filter(
    ctx: &AppContext,
) -> Result<Arc<dyn MarketplaceFilter>, ManagedError> {
    ctx.kernel()
        .state()
        .get::<Arc<MarketplaceSeams>>()
        .map(|seams| seams.filter())
        .map_err(unprovided)
}
