pub mod auth_provider;
pub mod cimd;
pub mod generation;
pub mod http;
pub mod jwt;
pub mod providers;
pub mod session;
pub mod templating;
pub mod validation;
pub mod webauthn;

pub use http::is_browser_request;
pub use jwt::{extract_bearer_token, extract_cookie_token, AuthService, TokenValidator};
pub use session::{
    AnonymousSessionInfo, CreateAnonymousSessionInput, SessionCreationError, SessionCreationService,
};
pub use templating::TemplateEngine;
pub use webauthn::{JwtTokenValidator, UserCreationService, WebAuthnConfig, WebAuthnService};

pub use generation::{
    generate_access_token_jti, generate_admin_jwt, generate_anonymous_jwt, generate_client_secret,
    generate_jwt, generate_secure_token, hash_client_secret, verify_client_secret, JwtConfig,
    JwtSigningParams,
};

pub use validation::{
    validate_any_audience, validate_jwt_token, validate_required_audience, validate_service_access,
};

pub use auth_provider::{JwtAuthProvider, JwtAuthorizationProvider, TraitBasedAuthService};
