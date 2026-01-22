#![allow(clippy::module_name_repetitions)]

pub mod constants;
pub mod extension;
pub mod models;
pub mod queries;
pub mod repository;
pub mod services;
pub mod state;

pub use extension::OauthExtension;

pub use models::*;
pub use repository::OAuthRepository;
pub use services::validation::jwt::validate_jwt_token;
pub use services::{
    extract_bearer_token, extract_cookie_token, is_browser_request, AnonymousSessionInfo,
    CreateAnonymousSessionInput, JwtAuthProvider, JwtAuthorizationProvider, SessionCreationService,
    TemplateEngine, TokenValidator, TraitBasedAuthService,
};
pub use services::providers::JwtValidationProviderImpl;

pub use state::OAuthState;
pub use systemprompt_models::auth::{AuthError, AuthenticatedUser, BEARER_PREFIX};
