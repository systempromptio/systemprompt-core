//! Per-flavour context middleware.
//!
//! Typed sibling middlewares that build a `RequestContext` for a route group,
//! with each type encoding its own caller-admission contract at the type level
//! rather than via a runtime `ContextRequirement` enum.
//!
//! Four flavours exist:
//!
//! - [`PublicContextMiddleware`] — admits `UserType::Anon`; forwards the
//!   session-derived `RequestContext` minted by `POST /oauth/session` and
//!   merges optional `x-context-id` / `x-agent-name` headers on top. Never
//!   reads or rebuilds the body.
//! - [`UserOnlyContextMiddleware`] — extracts a real user from headers; on
//!   extraction failure the request fails. Used for non-A2A authenticated
//!   routes.
//! - [`A2AContextMiddleware`] — extracts a real user AND parses the JSON-RPC
//!   body to recover `contextId` (the A2A wire spec carries it in the body, not
//!   headers). Rebuilds the body for downstream handlers.
//! - [`McpContextMiddleware`] — headers-only extraction; on extraction failure,
//!   forwards the session-derived `RequestContext` (Anon) so the downstream MCP
//!   proxy handler can answer with an RFC 9728 `WWW-Authenticate` 401
//!   challenge. The fallback is load-bearing — see
//!   `crates/tests/integration/api/routes_mcp_unauth_challenge.rs`.
//!
//! All four share the same `Arc<dyn ContextExtractor>` and the same error
//! mapping (`extraction_error_to_api_error`). Mounting a route under the
//! wrong flavour is a type error, not a runtime branch.

mod error;
mod flavours;
mod support;

pub use flavours::{
    A2AContextMiddleware, McpContextMiddleware, PublicContextMiddleware, UserOnlyContextMiddleware,
};
