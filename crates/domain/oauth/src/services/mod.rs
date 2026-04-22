pub mod cimd;
pub mod cowork;
pub mod generation;
pub mod http;
pub mod jwt;
pub mod providers;
pub mod session;
pub mod templating;
pub mod validation;
pub mod webauthn;

pub use cowork::{
    CoworkAuthResult, CoworkExchangeCode, exchange_cowork_session_code, hash_exchange_code,
    issue_cowork_access, issue_cowork_access_with, issue_cowork_exchange_code,
};
pub use http::is_browser_request;
pub use jwt::{AuthService, TokenValidator, extract_bearer_token, extract_cookie_token};
pub use session::{
    AnonymousSessionInfo, CreateAnonymousSessionInput, SessionCreationError, SessionCreationService,
};
pub use templating::TemplateEngine;
pub use webauthn::{JwtTokenValidator, UserCreationService, WebAuthnConfig, WebAuthnService};

pub use generation::{
    JwtConfig, JwtSigningParams, generate_access_token_jti, generate_admin_jwt,
    generate_admin_jwt_with_expiry, generate_anonymous_jwt, generate_anonymous_jwt_with_expiry,
    generate_client_secret, generate_jwt, generate_secure_token, hash_client_secret,
    verify_client_secret,
};

pub use validation::{
    validate_any_audience, validate_jwt_token, validate_required_audience, validate_service_access,
    verify_client_authentication,
};
