//! [`crate::AiService`] internals.
//!
//! Split across the per-endpoint client set, generation, streaming, tool execution, planning, the
//! [`systemprompt_models::ai::AiProvider`] bridge, and the streaming storage
//! wrapper.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod clients;
mod generation;
mod planning;
mod provider_impl;
mod service;
mod stream_wrapper;
mod streaming;
mod tool_execution;

pub use clients::{ProviderClients, WithheldEndpoint};
pub use service::{AiService, AiServiceProviders};
