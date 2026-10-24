//! HTTP middleware layers for the API server.
//!
//! Collects the request-pipeline concerns mounted by the router: client-address
//! resolution, CORS and security headers, bot detection and IP banning, rate
//! limiting, JWT and session context establishment, request-context flavours,
//! content negotiation, authorization gating, analytics, and tracing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod analytics;
pub mod authz;
pub mod bot_detector;
pub mod caller_purpose;
pub mod client_addr;
pub mod context;
pub mod cors;
pub mod ip_ban;
pub mod jwt;
pub mod load_shed;
pub mod negotiation;
pub mod rate_limit;
pub mod security_headers;
pub mod served_by;
pub mod session;
pub mod site_auth;
pub mod trace;
pub mod trailing_slash;

pub use analytics::{AnalyticsEventParams, AnalyticsMiddleware};
pub use authz::{AuthzPolicy, authz_gate};
pub use bot_detector::{
    BotMarker, BotType, detect_bots_early, is_datacenter_ip, is_known_bot, is_outdated_browser,
    is_scanner_request,
};
pub use caller_purpose::strip_caller_purpose;
pub use context::{
    A2AContextMiddleware, ContextExtractor, McpContextMiddleware, PublicContextMiddleware,
    UserOnlyContextMiddleware,
};
pub use cors::{CorsError, CorsMiddleware};
pub use ip_ban::ip_ban_middleware;
pub use jwt::{JtiRevocationChecker, JwtContextExtractor, JwtUserContext};
pub use load_shed::LoadShed;
pub use negotiation::{
    AcceptedFormat, AcceptedMediaType, content_negotiation_middleware, parse_accept_header,
};
pub use rate_limit::{ContextLayer, IdentityOrTrustedIpKey, RateLimitState, RouterExt};
pub use security_headers::inject_security_headers;
pub use served_by::inject_served_by;
pub use session::{
    SessionAttestationError, SessionMiddleware, attest_session, should_skip_session_tracking,
};
pub use site_auth::{login_redirect, site_auth_gate};
pub use trace::inject_trace_header;
pub use trailing_slash::{remove_trailing_slash, should_redirect};
