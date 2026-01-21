#![allow(clippy::module_name_repetitions)]

pub mod api;
pub mod models;
pub mod queries;
pub mod repository;
pub mod services;

pub use models::*;
pub use repository::OAuthRepository;
pub use services::validation::jwt::validate_jwt_token;
pub use services::{
    extract_bearer_token, extract_cookie_token, AnonymousSessionInfo, BrowserRedirectService,
    CreateAnonymousSessionInput, JwtAuthProvider, JwtAuthorizationProvider, SessionCreationService,
    TemplateEngine, TokenValidator, TraitBasedAuthService,
};

pub use systemprompt_models::auth::{AuthError, AuthenticatedUser, BEARER_PREFIX};
