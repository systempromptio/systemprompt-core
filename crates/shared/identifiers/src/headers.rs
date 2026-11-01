//! Canonical HTTP header name constants used throughout the platform for
//! propagating trace, session, and authorization context across services.
//!
//! `CALLER_PURPOSE` is the caller purpose sent to the credential broker, set
//! only from a credential the issuer bound it to; an inbound copy is stripped
//! at the API edge. `MCP_SERVER` names the MCP server a credential-broker
//! request is for. `API_KEY` is the Anthropic-style credential header a client
//! or upstream presents, and `OPENCODE_SESSION` the per-session header an
//! `OpenCode` client sends and a metered provider reads. `MEMORY` is the
//! request header a client sends as `off` to opt out of team-memory recall,
//! and `MEMORY_IDS` the response header naming the facts that were injected.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const TRACE_ID: &str = "x-trace-id";
pub const TRACEPARENT: &str = "traceparent";
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
pub const API_KEY: &str = "x-api-key";
pub const OPENCODE_SESSION: &str = "x-opencode-session";
pub const AUTHORIZATION: &str = "authorization";
pub const PROXY_VERIFIED: &str = "x-proxy-verified";
pub const USER_PERMISSIONS: &str = "x-user-permissions";
pub const USER_ROLES: &str = "x-user-roles";
pub const CLIENT_KIND: &str = "x-systemprompt-client";
pub const OWNER_HOP: &str = "x-systemprompt-owner-hop";
pub const CLIENT_ATTESTATION: &str = "x-systemprompt-client-attestation";
pub const SCOPE_PREFIX: &str = "x-systemprompt-scope-";
pub const CALLER_PURPOSE: &str = "x-systemprompt-caller";
pub const MCP_SERVER: &str = "x-systemprompt-mcp-server";
pub const MEMORY: &str = "x-systemprompt-memory";
pub const MEMORY_IDS: &str = "x-systemprompt-memory-ids";
pub const COLLAB_FORWARDED: &str = "x-systemprompt-collab-forwarded";
