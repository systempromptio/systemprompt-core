//! HTTP-facing middleware for the MCP server: RBAC enforcement, database-backed
//! session management, and request-context/bearer-token extraction helpers.

pub mod rbac;
pub mod session_handler;

use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer};
use systemprompt_models::RequestContext as SysRequestContext;
use systemprompt_traits::ContextPropagation;

pub use rbac::{AuthResult, AuthenticatedRequestContext, enforce_rbac_from_registry};
pub use session_handler::{DatabaseSessionHandler, PostgresSessionStore};

pub fn extract_bearer_token(
    mcp_context: &RequestContext<RoleServer>,
) -> Result<Option<String>, McpError> {
    let parts = mcp_context
        .extensions
        .get::<http::request::Parts>()
        .ok_or_else(|| {
            McpError::invalid_request("No HTTP parts in MCP context".to_owned(), None)
        })?;

    Ok(bearer_token_from_parts(parts))
}

pub fn bearer_token_from_parts(parts: &http::request::Parts) -> Option<String> {
    let auth_header = parts
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    if let Some(auth) = auth_header
        && let Some(token) = auth.strip_prefix("Bearer ")
    {
        return Some(token.to_owned());
    }

    None
}

pub fn extract_request_context(
    mcp_context: &RequestContext<RoleServer>,
) -> Result<SysRequestContext, McpError> {
    mcp_context
        .extensions
        .get::<http::request::Parts>()
        .map_or_else(
            || {
                Err(McpError::invalid_request(
                    "RequestContext missing - no axum parts in MCP context",
                    None,
                ))
            },
            request_context_from_parts,
        )
}

pub fn request_context_from_parts(
    parts: &http::request::Parts,
) -> Result<SysRequestContext, McpError> {
    if let Some(request_context) = parts.extensions.get::<SysRequestContext>() {
        return Ok(request_context.clone());
    }

    SysRequestContext::from_headers(&parts.headers)
        .map_err(|e| McpError::invalid_request(e.to_string(), None))
}
