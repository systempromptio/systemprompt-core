//! Route declarations: where a member's routes mount and the shape each one
//! carries.
//!
//! The value types — [`BasePath`], [`RouteSpec`], [`RouteShape`],
//! [`Allowance`], [`GatewayPath`] — are axum-free and always compiled, because
//! the kernel's privileged-shape constants name them. With the `router`
//! feature, a member builds a [`RouteSet`] at one base from typed handlers and
//! hands it to `Contributions::routes`; the HTTP error, content-negotiation
//! and server-sent-event helpers a capability's handlers use live here too,
//! so no capability entry crate depends on the API server.
//!
//! A base is a closed enum of `ApiPaths` constants. The one open variant,
//! [`ApiBase::Extension`], is validated when the set is built: it starts with
//! `/api/`, is a well-formed route path, and is not under any
//! [`RESERVED_API_PREFIXES`](crate::reserved::RESERVED_API_PREFIXES) entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "router")]
mod capability;
#[cfg(feature = "router")]
mod http_error;
#[cfg(feature = "router")]
mod negotiation;
#[cfg(feature = "router")]
mod principal;
#[cfg(feature = "router")]
mod set;
mod shape;
#[cfg(feature = "router")]
mod sse;

#[cfg(feature = "router")]
pub use capability::{IntoApiError, capability_response};
#[cfg(feature = "router")]
pub use http_error::{ApiHttpError, unreadable_body};
#[cfg(feature = "router")]
pub use negotiation::{
    AcceptedFormat, AcceptedMediaType, content_negotiation_middleware, parse_accept_header,
};
#[cfg(feature = "router")]
pub use principal::GatewayPrincipal;
#[cfg(feature = "router")]
pub use set::{GatewayRouteDecl, RouteSet, RouteSetBuilder};
pub use shape::{
    Allowance, ApiBase, BasePath, BaseRef, BodyLimit, GatewayPath, OperationSpec, RouteDef,
    RouteLimits, RoutePath, RouteShape, RouteSpec, TypedSpec, WebBase,
};
#[cfg(feature = "router")]
pub use sse::{
    HEARTBEAT_INTERVAL, HEARTBEAT_JSON, sse_response, standard_keep_alive, subscription_refused,
};

#[cfg(feature = "router")]
#[doc(hidden)]
pub use axum::response::{IntoResponse as __IntoResponse, Response as __Response};
