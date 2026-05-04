//! Canonical HTTP header name constants used throughout the platform for
//! propagating trace, session, and authorization context across services.

/// Trace identifier propagation header.
pub const TRACE_ID: &str = "x-trace-id";
/// Execution-context identifier propagation header.
pub const CONTEXT_ID: &str = "x-context-id";
/// Session identifier propagation header.
pub const SESSION_ID: &str = "x-session-id";
/// Authenticated user identifier propagation header.
pub const USER_ID: &str = "x-user-id";
/// Authenticated user type/role propagation header.
pub const USER_TYPE: &str = "x-user-type";
/// Task identifier propagation header.
pub const TASK_ID: &str = "x-task-id";
/// Agent name propagation header.
pub const AGENT_NAME: &str = "x-agent-name";
/// AI tool-call identifier propagation header.
pub const AI_TOOL_CALL_ID: &str = "x-ai-tool-call-id";
/// Originating call source propagation header.
pub const CALL_SOURCE: &str = "x-call-source";
/// OAuth client identifier propagation header.
pub const CLIENT_ID: &str = "x-client-id";
/// Tenant identifier propagation header.
pub const TENANT_ID: &str = "x-tenant-id";
/// Active policy version propagation header.
pub const POLICY_VERSION: &str = "x-policy-version";
/// Standard HTTP `Authorization` header name (lowercase).
pub const AUTHORIZATION: &str = "authorization";
/// Reverse-proxy verification flag header.
pub const PROXY_VERIFIED: &str = "x-proxy-verified";
/// Resolved-permission propagation header.
pub const USER_PERMISSIONS: &str = "x-user-permissions";
