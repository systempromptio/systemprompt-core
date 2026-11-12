//! Canonical HTTP header name constants used throughout the platform for
//! propagating trace, session, and authorization context across services.
//!
//! `MCP_HOP` carries the signed token the API proxy mints for each request it
//! forwards to a managed MCP server; it is the only identity that server
//! accepts. `CALLER_PURPOSE` is the caller purpose sent to the credential
//! broker, set only from a credential the issuer bound it to; an inbound copy
//! is stripped at the API edge. `MCP_SERVER` names the MCP server a
//! credential-broker request is for. `API_KEY` is the Anthropic-style
//! credential header a client or upstream presents, and `OPENCODE_SESSION` the
//! per-session header an `OpenCode` client sends and a metered provider reads.
//! `MEMORY` is the request header a client sends as `off` to opt out of
//! team-memory recall, and `MEMORY_IDS` the response header naming the facts
//! that were injected. `PLATFORM_PREFIX` is the namespace every
//! platform-owned `x-systemprompt-*` header lives in, for code that withholds
//! the whole class rather than one name.
//!
//! [`ALL`] lists every header name defined here and [`PREFIXES`] every
//! namespace, so a boundary that must not leak this platform's headers (a
//! request sent upstream with its client's own headers) withholds them by
//! reading the lists. [`CLIENT_TO_VENDOR`] is the part of [`ALL`] a client
//! addresses to the upstream itself, which such a request keeps. The MCP and
//! A2A names are those protocols' own; they are listed because this platform
//! speaks them between its processes.
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
pub const SYSTEMPROMPT_AUTHORIZATION: &str = "x-systemprompt-authorization";
pub const CLIENT_KIND: &str = "x-systemprompt-client";
pub const OWNER_HOP: &str = "x-systemprompt-owner-hop";
pub const CLIENT_ATTESTATION: &str = "x-systemprompt-client-attestation";
pub const PLATFORM_PREFIX: &str = "x-systemprompt-";
pub const SCOPE_PREFIX: &str = "x-systemprompt-scope-";
pub const CALLER_PURPOSE: &str = "x-systemprompt-caller";
pub const MCP_SERVER: &str = "x-systemprompt-mcp-server";
pub const MCP_HOP: &str = "x-systemprompt-mcp-hop";
pub const MEMORY: &str = "x-systemprompt-memory";
pub const MEMORY_IDS: &str = "x-systemprompt-memory-ids";
pub const COLLAB_FORWARDED: &str = "x-systemprompt-collab-forwarded";
pub const INGESTION_EVENT_ID: &str = "x-ingestion-event-id";
pub const BRIDGE_MANIFEST_SCHEMA: &str = "x-bridge-manifest-schema";
pub const UPLOAD_TOKEN: &str = "x-upload-token";
pub const MCP_SESSION_ID: &str = "mcp-session-id";
pub const MCP_PROTOCOL_VERSION: &str = "mcp-protocol-version";
pub const MCP_METHOD: &str = "mcp-method";
pub const A2A_VERSION: &str = "a2a-version";

pub const ALL: &[&str] = &[
    TRACE_ID,
    TRACEPARENT,
    CONTEXT_ID,
    GATEWAY_CONVERSATION_ID,
    PROVIDER_REQUEST_ID,
    SESSION_ID,
    USER_ID,
    USER_TYPE,
    TASK_ID,
    AGENT_NAME,
    AI_TOOL_CALL_ID,
    CALL_SOURCE,
    CLIENT_ID,
    POLICY_VERSION,
    INFERENCE_PROTOCOL,
    API_KEY,
    OPENCODE_SESSION,
    AUTHORIZATION,
    SYSTEMPROMPT_AUTHORIZATION,
    CLIENT_KIND,
    OWNER_HOP,
    CLIENT_ATTESTATION,
    CALLER_PURPOSE,
    MCP_SERVER,
    MCP_HOP,
    MEMORY,
    MEMORY_IDS,
    COLLAB_FORWARDED,
    INGESTION_EVENT_ID,
    BRIDGE_MANIFEST_SCHEMA,
    UPLOAD_TOKEN,
    MCP_SESSION_ID,
    MCP_PROTOCOL_VERSION,
    MCP_METHOD,
    A2A_VERSION,
];

pub const PREFIXES: &[&str] = &[PLATFORM_PREFIX, SCOPE_PREFIX];

pub const CLIENT_TO_VENDOR: &[&str] = &[OPENCODE_SESSION];
