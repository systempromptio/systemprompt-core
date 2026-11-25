//! The hosted A2A server: the HTTP edge of one spawned agent process.
//!
//! [`Server`] builds the axum router exposing the agent card and the A2A
//! JSON-RPC endpoint behind the agent's OAuth middleware ([`middleware`]);
//! [`request`] dispatches each JSON-RPC request and turns the agent
//! domain's stream frames into SSE events ([`sse_event`]); [`version`]
//! negotiates the A2A protocol version. [`serve::HostedAgentServer`] is the
//! `DynAgentServer` the `agent-http` member provides, through which the
//! kernel's `admin agent-host run` child host serves one agent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod card;
pub mod middleware;
pub mod request;
pub mod serve;
pub mod server;
pub mod standalone;
pub mod version;

use axum::response::sse::Event;
use systemprompt_agent::services::a2a_server::streaming::StreamFrame;

pub use server::Server;
pub use standalone::run_standalone;

#[must_use]
pub fn sse_event(frame: StreamFrame) -> Event {
    Event::default().data(frame.into_data())
}
