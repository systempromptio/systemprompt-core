//! [`crate::AiService`] internals.
//!
//! Split across generation, streaming, tool execution, planning, the
//! [`crate::AiProvider`] bridge, and the streaming storage wrapper.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod generation;
mod planning;
mod provider_impl;
mod service;
mod stream_wrapper;
mod streaming;
mod tool_execution;

pub use service::AiService;
