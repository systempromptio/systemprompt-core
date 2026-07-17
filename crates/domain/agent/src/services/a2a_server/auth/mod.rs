//! OAuth2/JWT authentication for the A2A server surface.
//!
//! Wires the Axum middleware ([`agent_oauth_middleware`]) to the token
//! validation routines ([`validate_agent_token`],
//! [`validate_oauth_for_request`]) and the shared [`AgentOAuthState`] /
//! [`AgentOAuthConfig`] carried on requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod middleware;
pub mod types;
pub mod validation;

pub use middleware::{agent_oauth_middleware, agent_oauth_middleware_wrapper, get_user_context};
pub use types::{AgentOAuthConfig, AgentOAuthState};
pub use validation::{extract_bearer_token, validate_agent_token, validate_oauth_for_request};
