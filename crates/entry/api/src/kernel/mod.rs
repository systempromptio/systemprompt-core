//! The helper surface a capability's route modules use from the API server.
//!
//! Route modules — under `routes/` today, in each capability's own entry
//! crate later — reach the server only through this module; it re-exports
//! and defines nothing. It carries:
//!
//! - the server's own handler error: [`ApiHttpError`] and [`unreadable_body`];
//! - request identity: [`JwtContextExtractor`] (the adapter over the kernel's
//!   `DynRequestAuthenticator`, built by `JwtContextExtractor::from_kernel`),
//!   the [`JwtUserContext`] it yields, and session attestation
//!   ([`attest_session`], [`SessionAttestationError`]);
//! - rate limiting: [`RateLimitState`], [`RouterExt`] and the in-handler
//!   [`SubjectThrottle`];
//! - the client address: [`resolve_client_ip`], the one resolver, and its
//!   [`ClientIp`] extractor;
//! - the session cookie ([`SessionCookie`], [`SameSite`]) and the request base
//!   URL ([`RequestBaseUrl`], [`resolve_request_base_url`]);
//! - server-sent events: [`create_sse_stream`] registers a connection with a
//!   user's broadcaster and streams its events;
//! - the shared proxy service: [`ProxyService`], [`ProxyTarget`],
//!   [`ProxyKind`], [`ProxyError`].
//!
//! The composition contract a member declares against (`RouteSet`,
//! `RouteSpec`, `ApiHttpError`, `capability_error_response!`,
//! `AcceptedFormat`, `sse_response`, `read_body_bounded`) is
//! `systemprompt_extension::router`'s
//! and is not repeated here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub use crate::error::{ApiHttpError, unreadable_body};
pub use crate::routes::stream::create_sse_stream;
pub use crate::services::middleware::client_addr::{ClientIp, resolve_client_ip};
pub use crate::services::middleware::{
    JwtContextExtractor, JwtUserContext, RateLimitState, RouterExt, SessionAttestationError,
    SubjectThrottle, attest_session,
};
pub use crate::services::proxy::{ProxyError, ProxyKind, ProxyService, ProxyTarget};
pub use crate::services::request_base_url::{RequestBaseUrl, resolve as resolve_request_base_url};
pub use crate::services::session_cookie::{SameSite, SessionCookie};
