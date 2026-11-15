//! A member's routes at one base, built from typed handlers.
//!
//! [`RouteSet::at`] names the base, the route group and the state type `S`
//! every handler extracts; [`RouteSetBuilder::route`] adds one method and path
//! with its [`RouteSpec`]. The set stores the handlers type-erased; the API
//! mount resolves `S` through its hand-written [`FromState`] impl only after
//! admission, and builds one router per declared route so it can apply each
//! route's own spec. A malformed path, an unsupported method or an invalid
//! extension base is recorded on the set and refused when the composition
//! reads the member's contributions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use axum::handler::Handler;
use axum::routing::{MethodFilter, MethodRouter};
use systemprompt_provider_contracts::MissingDependency;

use super::shape::{BasePath, GatewayPath, RouteDef, RouteLimits, RoutePath, RouteSpec, TypedSpec};
use crate::contribute::{ContributionError, FromState, Resolved};
use crate::protection::RouteGroup;

type Materialize =
    Arc<dyn Fn(&Resolved<'_>) -> Result<Vec<Router>, MissingDependency> + Send + Sync>;

pub struct RouteSet {
    base: BasePath,
    group: RouteGroup,
    defs: Vec<RouteDef>,
    materialize: Materialize,
    error: Option<ContributionError>,
}

impl RouteSet {
    pub fn at<S: FromState + Clone>(base: BasePath, group: RouteGroup) -> RouteSetBuilder<S> {
        RouteSetBuilder {
            base,
            group,
            routes: Vec::new(),
            error: base.validate().err(),
        }
    }

    #[must_use]
    pub fn defs(&self) -> &[RouteDef] {
        &self.defs
    }

    #[must_use]
    pub const fn base(&self) -> BasePath {
        self.base
    }

    #[must_use]
    pub const fn group(&self) -> RouteGroup {
        self.group
    }

    /// Resolves the set's state and builds one router per declared route, in
    /// [`RouteSet::defs`] order.
    pub fn materialize(&self, r: &Resolved<'_>) -> Result<Vec<Router>, MissingDependency> {
        (self.materialize)(r)
    }

    pub(crate) fn take_error(&mut self) -> Option<ContributionError> {
        self.error.take()
    }
}

impl std::fmt::Debug for RouteSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouteSet")
            .field("base", &self.base)
            .field("group", &self.group)
            .field("defs", &self.defs)
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

pub struct RouteSetBuilder<S> {
    base: BasePath,
    group: RouteGroup,
    routes: Vec<(RouteDef, MethodRouter<S>)>,
    error: Option<ContributionError>,
}

impl<S: FromState + Clone> RouteSetBuilder<S> {
    pub fn route<H, T>(self, m: http::Method, p: &'static str, h: H, spec: RouteSpec) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.push(m, p, h, spec, None)
    }

    pub fn route_typed<H, T>(self, m: http::Method, p: &'static str, h: H, spec: TypedSpec) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.push(m, p, h, spec.spec, Some(spec.openapi))
    }

    fn push<H, T>(
        mut self,
        m: http::Method,
        p: &'static str,
        h: H,
        spec: RouteSpec,
        openapi: Option<super::shape::OperationSpec>,
    ) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        if self.error.is_some() {
            return self;
        }
        let Ok(filter) = MethodFilter::try_from(m.clone()) else {
            self.error = Some(ContributionError::UnsupportedMethod(m));
            return self;
        };
        let path = match RoutePath::parse(p) {
            Ok(path) => path,
            Err(error) => {
                self.error = Some(error);
                return self;
            },
        };
        let def = RouteDef {
            method: m,
            path,
            spec,
            openapi,
        };
        self.routes.push((def, axum::routing::on(filter, h)));
        self
    }

    pub fn build(self) -> RouteSet {
        let (defs, routers): (Vec<RouteDef>, Vec<(&'static str, MethodRouter<S>)>) = self
            .routes
            .into_iter()
            .map(|(def, router)| {
                let path = def.path.as_str();
                (def, (path, router))
            })
            .unzip();
        let materialize: Materialize = Arc::new(move |r: &Resolved<'_>| {
            let state = S::from_state(r)?;
            Ok(routers
                .iter()
                .map(|(path, router)| {
                    Router::<S>::new()
                        .route(path, router.clone())
                        .with_state::<()>(state.clone())
                })
                .collect())
        });
        RouteSet {
            base: self.base,
            group: self.group,
            defs,
            materialize,
            error: self.error,
        }
    }
}

impl<S> std::fmt::Debug for RouteSetBuilder<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouteSetBuilder")
            .field("base", &self.base)
            .field("group", &self.group)
            .field("routes", &self.routes.len())
            .finish_non_exhaustive()
    }
}

/// A gateway route claim: its closed gateway path, limits and routes.
#[derive(Debug)]
pub struct GatewayRouteDecl {
    pub path: GatewayPath,
    pub limits: RouteLimits,
    pub router: RouteSet,
}
