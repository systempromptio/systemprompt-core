//! The site-fallback slot: the one service a composition answers unmatched
//! paths with.
//!
//! A member contributes at most one [`SiteFallback`]; the composition admits
//! one per composition and refuses a second claimant naming both. The API
//! mount places the admitted fallback as the tree's root fallback, after every
//! reserved path and every route set, under the public policy and the public
//! context only, and answers `404` from it for any request whose
//! percent-decoded path falls under a reserved entry
//! ([`super::reserved_answering`]). A reserved route that matches the path
//! but not the method still answers its own `405`.
//!
//! A fallback is either a plain service built while contributing
//! ([`SiteFallback::service`]) or one built from the member's own route
//! state when the API mounts it ([`SiteFallback::with_state`]): the state
//! type `S` resolves through the same member-scoped resolver as the member's
//! route sets, so an install's fallback state reads only what an install's
//! route state may read and never a host-sealed seed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::convert::Infallible;
use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::response::IntoResponse;
use systemprompt_provider_contracts::MissingDependency;
use tower::Service;

use crate::contribute::{FromState, Resolved};

type BuildFallback = Arc<dyn Fn(&Resolved<'_>) -> Result<Router, MissingDependency> + Send + Sync>;

/// The service a composition answers unmatched paths with, under the public
/// policy only.
#[derive(Clone)]
pub struct SiteFallback {
    build: BuildFallback,
}

impl SiteFallback {
    pub fn service<T>(service: T) -> Self
    where
        T: Service<Request, Error = Infallible> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        Self {
            build: Arc::new(move |_: &Resolved<'_>| {
                Ok(Router::new().fallback_service(service.clone()))
            }),
        }
    }

    pub fn with_state<S, T, F>(build: F) -> Self
    where
        S: FromState,
        F: Fn(S) -> T + Send + Sync + 'static,
        T: Service<Request, Error = Infallible> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        Self {
            build: Arc::new(move |r: &Resolved<'_>| {
                Ok(Router::new().fallback_service(build(S::from_state(r)?)))
            }),
        }
    }

    pub fn materialize(&self, r: &Resolved<'_>) -> Result<Router, MissingDependency> {
        (self.build)(r)
    }
}

impl std::fmt::Debug for SiteFallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SiteFallback").finish_non_exhaustive()
    }
}
