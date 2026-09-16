//! JSON-RPC 2.0 error construction for the A2A server.
//!
//! Exposes [`JsonRpcErrorBuilder`] for assembling spec-compliant error
//! envelopes and the [`unauthorized_response`] / [`forbidden_response`]
//! helpers for auth failures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod jsonrpc;

pub use jsonrpc::{JsonRpcErrorBuilder, forbidden_response, unauthorized_response};
