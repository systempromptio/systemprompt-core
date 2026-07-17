//! `WebAuthn` (passkey) registration and authentication services.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod config;
pub mod jwt;
pub mod registry;
pub mod service;
pub mod token;
pub mod user_service;

pub use config::WebAuthnConfig;
pub use jwt::JwtTokenValidator;
pub use registry::WebAuthnRegistry;
pub use service::{
    FinishRegistrationParams, LinkStates, LinkUserInfo, WebAuthnService, create_link_states,
};
pub use token::{generate_setup_token, hash_token, validate_token_format};
pub use user_service::UserCreationService;
