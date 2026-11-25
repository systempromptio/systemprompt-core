//! OAuth2/JWT authentication for the A2A server surface.
//!
//! The token validation routine ([`validate_oauth_for_request`]) and the
//! shared [`AgentOAuthState`] / [`AgentOAuthConfig`] the A2A server's OAuth
//! middleware carries on requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod types;
pub mod validation;

pub use types::{AgentOAuthConfig, AgentOAuthState};
pub use validation::{AuthenticatedCaller, extract_bearer_token, validate_oauth_for_request};
