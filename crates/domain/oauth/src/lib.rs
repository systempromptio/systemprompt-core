#![allow(unused_qualifications)]
#![allow(clippy::similar_names)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::if_not_else)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::redundant_else)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_fields_in_debug)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::ref_option)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::unused_async)]
#![allow(clippy::single_match_else)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::struct_excessive_bools)]

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
