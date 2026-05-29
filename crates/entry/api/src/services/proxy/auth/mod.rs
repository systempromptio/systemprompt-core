//! Proxy authentication and authorization for backend service access.
//!
//! Two cohesive halves:
//!
//! - `challenge`: validating credential presence and building the RFC 6750 /
//!   RFC 9728 `WWW-Authenticate` 401/403 OAuth challenges that drive MCP and
//!   agent clients into the OAuth discovery flow.
//! - `access`: resolving a service's OAuth requirement from the agent / MCP
//!   registries and enforcing the required scopes against the authenticated
//!   user, with the session-cache fallback for already-established MCP
//!   sessions.

mod access;
mod challenge;

pub(crate) use access::AccessValidator;
pub use challenge::OAuthChallengeBuilder;
pub(crate) use challenge::build_mcp_unknown_service_challenge;
