pub mod rbac;
pub mod session_manager;

use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer};
use systemprompt_models::RequestContext as SysRequestContext;
use systemprompt_traits::ContextPropagation;

pub use rbac::{enforce_rbac_from_registry, AuthResult, AuthenticatedRequestContext};
pub use session_manager::DatabaseSessionManager;

pub fn extract_bearer_token(
    mcp_context: &RequestContext<RoleServer>,
) -> Result<Option<String>, McpError> {
    let parts = mcp_context
        .extensions
        .get::<http::request::Parts>()
        .ok_or_else(|| {
            McpError::invalid_request("No HTTP parts in MCP context".to_string(), None)
        })?;

    let auth_header = parts
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    if let Some(auth) = auth_header {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            return Ok(Some(token.to_string()));
        }
    }

    Ok(None)
}

pub fn extract_request_context(
    mcp_context: &RequestContext<RoleServer>,
) -> Result<SysRequestContext, McpError> {
    let parts = mcp_context.extensions.get::<http::request::Parts>();

    if let Some(parts) = parts {
        if let Some(request_context) = parts.extensions.get::<SysRequestContext>() {
            return Ok(request_context.clone());
        }

        return SysRequestContext::from_headers(&parts.headers)
            .map_err(|e| McpError::invalid_request(e.to_string(), None));
    }

    Err(McpError::invalid_request(
        "RequestContext missing - no axum parts in MCP context",
        None,
    ))
}
