//! The managed consumer surface contract.
//!
//! The `marketplace-http` member contributes a [`ManagedSurface`]: one
//! builder the API mount calls with the route-scoped state of
//! `marketplace-http` ([`ManagedRouteState`]). The member owns the handlers,
//! the origin guard and the consumer contract normaliser; the API owns the
//! protection between them. [`ManagedRoutes::consumer`] carries the origin
//! guard innermost; the API protects it handler-verified by the device
//! credential under the `consumer_evidence` limit and hands the protected
//! router to [`ManagedRoutes::normalize`], which wraps it outermost so a
//! refusal from any inner layer is normalised too.
//!
//! [`ConsumerCatalog`] is the one read the handlers take from the host: the
//! marketplace candidate a consumer's bridge manifest resolves to, which the
//! resource authorization matches the requested publication against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use systemprompt_extension::prelude::{MissingDependency, Resolved};
use systemprompt_identifiers::UserId;
use systemprompt_traits::BoxedSource;

use crate::MarketplaceCandidate;

/// Builds the managed consumer routes from the `marketplace-http` route
/// state.
pub trait ManagedSurface: Send + Sync + std::fmt::Debug {
    fn build(&self, state: ManagedRouteState<'_>) -> Result<ManagedRoutes, MissingDependency>;
}

pub type DynManagedSurface = Arc<dyn ManagedSurface>;

/// The marketplace candidate a consumer's bridge manifest resolves to.
///
/// Injected as `dyn ConsumerCatalog` into the route state, hence
/// `#[async_trait]`.
#[async_trait::async_trait]
pub trait ConsumerCatalog: Send + Sync {
    async fn candidate(&self, consumer: &UserId) -> Result<MarketplaceCandidate, BoxedSource>;
}

pub type DynConsumerCatalog = Arc<dyn ConsumerCatalog>;

/// The state the API mount hands a [`ManagedSurface`].
#[derive(Debug)]
pub struct ManagedRouteState<'a> {
    pub resolved: Resolved<'a>,
    pub catalog: DynConsumerCatalog,
}

pub type ConsumerNormalizer = Box<dyn FnOnce(Router) -> Router + Send>;

/// The consumer router with its origin guard, and the normaliser the API
/// wraps the protected router in.
pub struct ManagedRoutes {
    pub consumer: Router,
    pub normalize: ConsumerNormalizer,
}

impl ManagedRoutes {
    pub fn protect<E>(
        self,
        protect: impl FnOnce(Router) -> Result<Router, E>,
    ) -> Result<Router, E> {
        Ok((self.normalize)(protect(self.consumer)?))
    }
}

impl std::fmt::Debug for ManagedRoutes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedRoutes").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for dyn ConsumerCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ConsumerCatalog")
    }
}
