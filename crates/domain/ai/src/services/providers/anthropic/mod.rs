//! Anthropic provider driver.
//!
//! Chat completions, streaming, search-grounded responses, and tool use.
//! Vendor wire translation is delegated to the shared
//! `systemprompt_models::wire::anthropic` codec via the `canonical_bridge`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod generation;
mod provider;
mod request;
pub mod search;
mod streaming;
mod trait_impl;

pub use provider::AnthropicProvider;
