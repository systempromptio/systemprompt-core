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
//! but not the method still answers its own `405`. The fallback is a plain
//! service the member builds while contributing, so it carries no kernel
//! handle and no host-sealed state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::convert::Infallible;

use axum::Router;
use axum::extract::Request;
use axum::response::IntoResponse;
use tower::Service;

/// The service a composition answers unmatched paths with, under the public
/// policy only.
#[derive(Clone)]
pub struct SiteFallback {
    router: Router,
}

impl SiteFallback {
    pub fn service<T>(service: T) -> Self
    where
        T: Service<Request, Error = Infallible> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        Self {
            router: Router::new().fallback_service(service),
        }
    }

    pub fn into_router(self) -> Router {
        self.router
    }
}

impl std::fmt::Debug for SiteFallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SiteFallback").finish_non_exhaustive()
    }
}
