//! HTTP server implementation of the A2A (agent-to-agent) protocol.
//!
//! Hosts the JSON-RPC endpoint and agent-card discovery via [`Server`] (or the
//! [`run_standalone`] entry point), with submodules for authentication
//! ([`auth`]), error envelopes ([`errors`]), request [`handlers`], and the
//! streaming and processing pipelines.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod auth;
pub mod errors;
pub mod handlers;
pub mod processing;
pub mod server;
pub mod standalone;
pub mod streaming;

pub use handlers::AgentHandlerState;
pub use server::Server;
pub use standalone::run_standalone;
pub use systemprompt_models::AgentConfig;
