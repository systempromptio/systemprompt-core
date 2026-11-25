//! The agent domain of the A2A (agent-to-agent) protocol.
//!
//! Authentication ([`auth`]), JSON-RPC error envelopes ([`errors`]), the
//! handler state ([`handlers`]) and the streaming and processing pipelines.
//! The streaming engine emits [`streaming::StreamFrame`]s; the HTTP edge
//! that serves them as SSE, the A2A JSON-RPC endpoint and protocol version
//! negotiation are `systemprompt-agent-entry`'s.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod active_tasks;
pub mod auth;
pub mod errors;
pub mod handlers;
pub mod processing;
pub mod streaming;

pub use active_tasks::{ActiveTaskGuard, ActiveTasks};
pub use handlers::AgentHandlerState;
pub use systemprompt_manifest::AgentConfig;

pub const AGENT_RUN_ORIGIN_KIND: &str = "a2a";
