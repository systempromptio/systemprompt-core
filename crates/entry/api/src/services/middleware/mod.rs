//! HTTP middleware layers for the API server.
//!
//! Collects the request-pipeline concerns mounted by the router: client-address
//! resolution, CORS and security headers, bot detection and IP banning, rate
//! limiting, JWT and session context establishment, request-context flavours,
//! content negotiation, authorization gating, analytics, and tracing.

pub mod analytics;
pub mod authz;
pub mod bot_detector;
pub mod client_addr;
pub mod context;
pub mod cors;
pub mod ip_ban;
pub mod jwt;
pub mod negotiation;
pub mod rate_limit;
pub mod security_headers;
pub mod served_by;
pub mod session;
pub mod site_auth;
pub mod trace;
pub mod trailing_slash;

pub use analytics::*;
pub use authz::{AuthzPolicy, authz_gate};
pub use bot_detector::*;
pub use context::{
    A2AContextMiddleware, ContextExtractor, McpContextMiddleware, PublicContextMiddleware,
    UserOnlyContextMiddleware,
};
pub use cors::*;
pub use ip_ban::*;
pub use jwt::{JtiRevocationChecker, JwtContextExtractor, JwtUserContext};
pub use negotiation::{
    AcceptedFormat, AcceptedMediaType, content_negotiation_middleware, parse_accept_header,
};
pub use rate_limit::*;
pub use security_headers::*;
pub use served_by::*;
pub use session::*;
pub use site_auth::*;
pub use trace::*;
pub use trailing_slash::*;
