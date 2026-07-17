//! Context-id sources for the context middleware.
//!
//! [`HeaderSource`] reads the context id from request headers;
//! [`PayloadSource`] recovers it from the JSON-RPC body (the A2A wire location)
//! while preserving the body for downstream handlers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod headers;
pub mod payload;

pub use headers::HeaderSource;
pub use payload::PayloadSource;
pub use systemprompt_models::execution::ContextIdSource;
