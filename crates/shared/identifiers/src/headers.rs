//! Canonical HTTP header name constants used throughout the platform for
//! propagating trace, session, and authorization context across services.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const TRACE_ID: &str = "x-trace-id";
pub const CONTEXT_ID: &str = "x-context-id";
pub const GATEWAY_CONVERSATION_ID: &str = "x-gateway-conversation-id";
pub const PROVIDER_REQUEST_ID: &str = "x-provider-request-id";
pub const SESSION_ID: &str = "x-session-id";
pub const USER_ID: &str = "x-user-id";
pub const USER_TYPE: &str = "x-user-type";
pub const TASK_ID: &str = "x-task-id";
pub const AGENT_NAME: &str = "x-agent-name";
pub const AI_TOOL_CALL_ID: &str = "x-ai-tool-call-id";
pub const CALL_SOURCE: &str = "x-call-source";
pub const CLIENT_ID: &str = "x-client-id";
pub const POLICY_VERSION: &str = "x-policy-version";
pub const INFERENCE_PROTOCOL: &str = "x-inference-protocol";
pub const AUTHORIZATION: &str = "authorization";
pub const PROXY_VERIFIED: &str = "x-proxy-verified";
pub const USER_PERMISSIONS: &str = "x-user-permissions";
