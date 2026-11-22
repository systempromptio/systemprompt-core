//! A member's routes at one base, built from typed handlers.
//!
//! [`RouteSet::at`] names the base, the route group and the state type `S`
//! every handler extracts; [`RouteSet::gateway`] names a closed gateway path
//! in place of the base, for a [`GatewayRouteDecl`]; [`RouteSetBuilder::route`]
//! adds one method and path with its [`RouteSpec`];
//! [`RouteSetBuilder::service`] nests one opaque service under a prefix,
//! recorded as a single [`RouteMethod::Any`] def. The set stores the handlers
//! type-erased; the API mount resolves `S` through its hand-written
//! [`FromState`] impl only after admission, and builds one router per declared
//! route so it can apply each route's own spec. A malformed path, an
//! unsupported method or an invalid extension base is recorded on the set and
//! refused when the composition reads the member's contributions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::convert::Infallible;
use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::handler::Handler;
use axum::response::IntoResponse;
use axum::routing::MethodFilter;
use systemprompt_provider_contracts::MissingDependency;
use tower::Service;

use super::base_ref::{BaseRef, GatewayPath};
use super::shape::{BasePath, RouteDef, RouteLimits, RouteMethod, RoutePath, RouteSpec, TypedSpec};
use crate::contribute::{ContributionError, FromState, Resolved};
use crate::protection::RouteGroup;

type Materialize =
    Arc<dyn Fn(&Resolved<'_>) -> Result<Vec<Router>, MissingDependency> + Send + Sync>;

pub struct RouteSet {
    base: BaseRef,
    group: RouteGroup,
    defs: Vec<RouteDef>,
    materialize: Materialize,
    error: Option<ContributionError>,
}

impl RouteSet {
    pub fn at<S: FromState + Clone>(base: BasePath, group: RouteGroup) -> RouteSetBuilder<S> {
        RouteSetBuilder {
            base: BaseRef::Path(base),
            group,
            routes: Vec::new(),
            error: base.validate().err(),
        }
    }

    pub const fn gateway<S: FromState + Clone>(
        path: GatewayPath,
        group: RouteGroup,
    ) -> RouteSetBuilder<S> {
        RouteSetBuilder {
            base: BaseRef::Gateway(path),
            group,
            routes: Vec::new(),
            error: None,
        }
    }

    #[must_use]
    pub fn defs(&self) -> &[RouteDef] {
        &self.defs
    }

    #[must_use]
    pub const fn base(&self) -> BaseRef {
        self.base
    }

    #[must_use]
    pub const fn group(&self) -> RouteGroup {
        self.group
    }

    pub fn materialize(&self, r: &Resolved<'_>) -> Result<Vec<Router>, MissingDependency> {
        (self.materialize)(r)
    }

    pub(crate) const fn take_error(&mut self) -> Option<ContributionError> {
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
    base: BaseRef,
    group: RouteGroup,
    routes: Vec<(RouteDef, Router<S>)>,
    error: Option<ContributionError>,
}

impl<S: FromState + Clone> RouteSetBuilder<S> {
    pub fn route<H, T>(self, m: http::Method, p: &'static str, h: H, spec: RouteSpec) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.push(m, p, h, (spec, None))
    }

    pub fn route_typed<H, T>(self, m: http::Method, p: &'static str, h: H, spec: TypedSpec) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        self.push(m, p, h, (spec.spec, Some(spec.openapi)))
    }

    fn push<H, T>(
        mut self,
        m: http::Method,
        p: &'static str,
        h: H,
        (spec, openapi): (RouteSpec, Option<super::shape::OperationSpec>),
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
        let router = Router::new().route(path.as_str(), axum::routing::on(filter, h));
        let def = RouteDef {
            method: RouteMethod::One(m),
            path,
            spec,
            openapi,
        };
        self.routes.push((def, router));
        self
    }

    pub fn service<T>(mut self, prefix: &'static str, service: T, spec: RouteSpec) -> Self
    where
        T: Service<Request, Error = Infallible> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        if self.error.is_some() {
            return self;
        }
        let path = match RoutePath::parse(prefix) {
            Ok(path) if path.is_service_prefix() => path,
            Ok(_) => {
                self.error = Some(ContributionError::InvalidPath(prefix));
                return self;
            },
            Err(error) => {
                self.error = Some(error);
                return self;
            },
        };
        let def = RouteDef {
            method: RouteMethod::Any,
            path,
            spec,
            openapi: None,
        };
        self.routes
            .push((def, Router::new().nest_service(prefix, service)));
        self
    }

    pub fn build(self) -> RouteSet {
        let (defs, routers): (Vec<RouteDef>, Vec<Router<S>>) = self.routes.into_iter().unzip();
        let materialize: Materialize = Arc::new(move |r: &Resolved<'_>| {
            let state = S::from_state(r)?;
            Ok(routers
                .iter()
                .map(|router| router.clone().with_state::<()>(state.clone()))
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

/// A gateway route claim: its limits and the routes built by
/// [`RouteSet::gateway`], whose gateway path is the claim's path.
#[derive(Debug)]
pub struct GatewayRouteDecl {
    pub limits: RouteLimits,
    pub router: RouteSet,
}

impl GatewayRouteDecl {
    #[must_use]
    pub const fn path(&self) -> Option<GatewayPath> {
        match self.router.base {
            BaseRef::Gateway(path) => Some(path),
            BaseRef::Path(_) => None,
        }
    }
}
