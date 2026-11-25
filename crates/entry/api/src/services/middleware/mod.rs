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
pub mod body;
pub mod bot_detector;
pub mod caller_purpose;
pub mod context;
pub mod cors;
pub mod ip_ban;
pub mod jwt;
pub mod load_shed;
pub mod mount;
pub mod panic_fence;
pub mod rate_limit;
pub mod security_headers;
pub mod served_by;
pub mod session;
pub mod site_auth;
pub mod trace;
pub mod trailing_slash;

pub use analytics::{AnalyticsEventParams, AnalyticsMiddleware};
pub use authz::authz_gate;
pub use body::{BodyBudget, BodyReservation};
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
pub use jwt::{JwtContextExtractor, JwtUserContext};
pub use mount::{
    ClientWritesGuard, DEFAULT_BODY_LIMIT_BYTES, KernelRoutes, MountError, MountLayers,
    MountedGatewayRoute, ProtectionGuards, RootFallback, RootFallbackOwner, SiteAuthGuard,
    gateway_protection, gateway_routes, mount, mount_admitted, path_under, place, protect,
    protect_root_fallback, protection_of, recheck,
};
pub use panic_fence::panic_fence;
pub use rate_limit::{
    ContextLayer, IdentityOrTrustedIpKey, LayerReach, MemoryRateLimitBuckets, RateLimitState,
    RouterExt, SubjectThrottle, Throttled, TrustedIpKey, with_pre_auth_ip_limit,
};
pub use security_headers::inject_security_headers;
pub use served_by::inject_served_by;
pub use session::{SessionMiddleware, should_skip_session_tracking};
pub use site_auth::{SiteAuthSession, login_redirect, site_auth_gate};
pub use systemprompt_security::{SessionAttestationError, attest_session};
pub use trace::inject_trace_header;
pub use trailing_slash::{remove_trailing_slash, should_redirect};
