//! Reverse-proxy routes to managed backends.
//!
//! Fronts the [`agents`] (A2A) and [`mcp`] services, forwarding requests
//! through the proxy engine and exposing their discovery metadata.

pub mod agents;
pub mod mcp;
