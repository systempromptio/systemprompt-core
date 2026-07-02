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

pub(crate) use access::{AccessValidator, mcp_oauth_requirement};
pub use challenge::OAuthChallengeBuilder;
pub(crate) use challenge::build_mcp_unknown_service_challenge;

#[cfg(feature = "test-api")]
pub mod test_api {
    use axum::http::HeaderMap;
    use axum::response::{IntoResponse, Response};
    use systemprompt_models::RequestContext;
    use systemprompt_models::auth::AuthenticatedUser;
    use systemprompt_runtime::AppContext;

    #[derive(Debug)]
    pub struct Requirement {
        pub module: String,
        pub required: bool,
        pub scopes: Vec<String>,
        pub audience: String,
    }

    pub fn validate_with_requirement(
        headers: &HeaderMap,
        service_name: &str,
        requirement: &Requirement,
        ctx: &AppContext,
        req_context: Option<&RequestContext>,
    ) -> Result<Option<AuthenticatedUser>, Box<Response>> {
        let internal = super::access::OAuthRequirement {
            module: requirement.module.clone(),
            required: requirement.required,
            scopes: requirement.scopes.clone(),
            audience: requirement.audience.clone(),
        };
        super::access::AccessValidator::validate_with_requirement(
            headers,
            service_name,
            &internal,
            ctx,
            req_context,
        )
        .map_err(|e| Box::new(e.into_response()))
    }
}
