//! Confining a router to the paths it may answer.
//!
//! axum does not let the server enumerate the routes a built `Router` holds,
//! so a bound on what a router serves is enforced on the request instead:
//! [`confined`] answers 404 to a request path outside every listed prefix,
//! [`pinned`] to a request whose method and path match none of a row's
//! [`RoutePin`]s, and [`excluding`] to a request under a path another row at
//! the same base path claims. The layer wraps only the routes of the router it
//! is applied to, before any other layer, so the request a handler sees has
//! already passed it; a route added outside the bound answers 404 and fails
//! its own route tests instead of escaping the check. Paths are matched
//! segment-bounded ([`path_under`]) against the path the router sees, which
//! for a nested router is relative to its base path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::IntoResponse;

use super::path_under;

/// The request method a [`RoutePin`] admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinnedMethod {
    Get,
    Post,
}

impl PinnedMethod {
    fn admits(self, method: &Method) -> bool {
        match self {
            Self::Get => *method == Method::GET || *method == Method::HEAD,
            Self::Post => *method == Method::POST,
        }
    }
}

/// One method and path prefix a core declaration row's router may answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutePin {
    pub method: PinnedMethod,
    pub prefix: &'static str,
}

impl RoutePin {
    #[must_use]
    pub const fn new(method: PinnedMethod, prefix: &'static str) -> Self {
        Self { method, prefix }
    }

    fn admits(&self, method: &Method, path: &str) -> bool {
        self.method.admits(method) && path_under(path, self.prefix)
    }
}

fn gate(
    router: Router,
    admits: impl Fn(&Method, &str) -> bool + Clone + Send + Sync + 'static,
) -> Router {
    router.layer(axum::middleware::from_fn(
        move |request: Request, next: Next| {
            let admits = admits.clone();
            async move {
                if admits(request.method(), request.uri().path()) {
                    next.run(request).await
                } else {
                    StatusCode::NOT_FOUND.into_response()
                }
            }
        },
    ))
}

pub fn confined(router: Router, prefixes: Vec<&'static str>) -> Router {
    let prefixes: Arc<[&'static str]> = prefixes.into();
    gate(router, move |_, path| {
        prefixes.iter().any(|prefix| path_under(path, prefix))
    })
}

pub fn pinned(router: Router, pins: &'static [RoutePin]) -> Router {
    gate(router, move |method, path| {
        pins.iter().any(|pin| pin.admits(method, path))
    })
}

pub fn excluding(router: Router, claimed: Vec<&'static str>) -> Router {
    let claimed: Arc<[&'static str]> = claimed.into();
    gate(router, move |_, path| {
        !claimed.iter().any(|prefix| path_under(path, prefix))
    })
}
