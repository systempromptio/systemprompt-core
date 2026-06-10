//! OAuth domain services: token generation, JWT, plugin-scoped token minting,
//! session, `WebAuthn`, CIMD, validation, templating.

pub mod bridge;
pub mod cimd;
pub mod generation;
pub mod http;
pub mod jwt;
pub mod plugin_token;
pub mod providers;
pub mod session;
pub mod templating;
pub mod validation;
pub mod webauthn;

pub use bridge::{
    BridgeAccessRequest, BridgeAuthResult, BridgeExchangeCode, BridgeOAuthClient,
    exchange_bridge_session_code, hash_exchange_code, issue_bridge_access,
    issue_bridge_access_with, issue_bridge_exchange_code, provision_bridge_oauth_client,
};
pub use http::is_browser_request;
pub use jwt::{AuthService, TokenValidator, extract_bearer_token, extract_cookie_token};
pub use plugin_token::{IssuedPluginToken, PluginTokenService, PluginTokenSubject};
pub use session::{
    AnonymousSessionInfo, CreateAnonymousSessionInput, SessionCreationError, SessionCreationService,
};
pub use templating::TemplateEngine;
pub use webauthn::{JwtTokenValidator, UserCreationService, WebAuthnConfig, WebAuthnService};

pub use generation::{
    JwtConfig, JwtSigningParams, generate_access_token_jti, generate_admin_jwt,
    generate_admin_jwt_with_expiry, generate_anonymous_jwt, generate_anonymous_jwt_with_expiry,
    generate_client_secret, generate_jwt, generate_jwt_with_act, generate_secure_token,
    hash_client_secret, verify_client_secret,
};

pub use validation::{
    validate_any_audience, validate_jwt_token, validate_required_audience, validate_service_access,
    verify_client_authentication,
};
