//! What one gateway request resolves to, before anything is dispatched.
//!
//! [`resolve_route`] asks the [`ResolvedCatalog`] for the request's model
//! (`model → deployment → endpoint → provider → credential`, an alias or
//! policy route applied) with the request's scope attribution and canonical
//! body, so `when:` predicates see exactly what dispatch sees. The result is
//! the one answer the API layer (route id for access control, the provider the
//! audit row opens on) and the dispatch path share, so the two cannot drift.
//!
//! Policy routes come from the catalog's `gateway.routes`; routes edited in
//! the database arrive with C-WP7c through [`RouteSource`], the one seam the
//! route table is read from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::RouteId;
use systemprompt_manifest::services::GatewayRoute;
use systemprompt_manifest::services::catalog::{
    ModelRequest, ResolveError, ResolvedCatalog, ResolvedHop, ResolvedTarget,
};
use systemprompt_models::attribution::RequestAttribution;
use systemprompt_wire::canonical::CanonicalRequest;

/// Where the gateway reads its policy routes from.
#[derive(Debug, Clone, Copy)]
pub enum RouteSource<'a> {
    Catalog(&'a ResolvedCatalog),
}

impl<'a> RouteSource<'a> {
    #[must_use]
    pub const fn catalog(self) -> &'a ResolvedCatalog {
        match self {
            Self::Catalog(catalog) => catalog,
        }
    }

    #[must_use]
    pub fn routes(self) -> &'a [GatewayRoute] {
        self.catalog().routes()
    }
}

/// One request's resolution: the catalog target and the route that shaped it.
#[derive(Debug, Clone)]
pub struct RouteResolution {
    pub target: ResolvedTarget,
}

impl RouteResolution {
    #[must_use]
    pub const fn access_route_id(&self) -> &RouteId {
        &self.target.access_route_id
    }

    #[must_use]
    pub fn primary(&self) -> Option<&ResolvedHop> {
        self.target.primary()
    }

    #[must_use]
    pub fn provider(&self) -> Option<&str> {
        self.primary().map(|hop| hop.provider.as_str())
    }

    #[must_use]
    pub fn route<'c>(&self, catalog: &'c ResolvedCatalog) -> Option<&'c GatewayRoute> {
        self.target
            .route
            .as_ref()
            .and_then(|id| catalog.route(id.as_str()))
    }
}

pub fn resolve_route(
    source: RouteSource<'_>,
    request: &CanonicalRequest,
    attribution: &RequestAttribution,
) -> Result<RouteResolution, ResolveError> {
    let model_request = ModelRequest::new(request.model.as_str())
        .with_attribution(attribution)
        .with_request(request);
    source
        .catalog()
        .resolve(&model_request)
        .map(|target| RouteResolution { target })
}
