//! JSON-RPC 2.0 error construction for the A2A server.
//!
//! Exposes [`JsonRpcErrorBuilder`] for assembling spec-compliant error
//! envelopes — each carrying a `google.rpc.ErrorInfo` ([`A2aErrorInfo`]) in
//! `error.data` — and the [`unauthorized_response`] / [`forbidden_response`]
//! helpers for auth failures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod error_info;
pub mod jsonrpc;

pub use error_info::{A2aErrorInfo, A2aErrorReason};
pub use jsonrpc::{
    JsonRpcErrorBuilder, JsonRpcErrorResponse, UnidentifiedErrorResponse, forbidden_response,
    unauthorized_response,
};
