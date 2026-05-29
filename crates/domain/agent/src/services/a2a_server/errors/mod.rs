//! JSON-RPC 2.0 error construction for the A2A server.
//!
//! Exposes [`JsonRpcErrorBuilder`] for assembling spec-compliant error
//! envelopes, the [`unauthorized_response`] / [`forbidden_response`] helpers
//! for auth failures, and [`classify_database_error`] for mapping repository
//! errors onto user-facing messages.

pub mod jsonrpc;

pub use jsonrpc::{
    JsonRpcErrorBuilder, classify_database_error, forbidden_response, unauthorized_response,
};
