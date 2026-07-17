//! Reverse-proxy routes to managed backends.
//!
//! Fronts the [`agents`] (A2A) and [`mcp`] services, forwarding requests
//! through the proxy engine and exposing their discovery metadata.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agents;
pub mod mcp;
