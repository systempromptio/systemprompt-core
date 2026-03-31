#![allow(clippy::module_name_repetitions)]

pub mod constants;
pub(crate) mod extension;
pub mod models;
pub(crate) mod queries;
pub mod repository;
pub mod services;
pub(crate) mod state;

pub use extension::OauthExtension;

pub use models::*;
pub use repository::OAuthRepository;
pub use services::providers::JwtValidationProviderImpl;
pub use services::validation::jwt::validate_jwt_token;
pub use services::{
    AnonymousSessionInfo, CreateAnonymousSessionInput, JwtAuthProvider, JwtAuthorizationProvider,
    SessionCreationError, SessionCreationService, TemplateEngine, TokenValidator,
    TraitBasedAuthService, extract_bearer_token, extract_cookie_token, is_browser_request,
};

pub use state::OAuthState;
pub use systemprompt_models::auth::{AuthError, AuthenticatedUser, BEARER_PREFIX};
