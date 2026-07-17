//! `OpenAI` provider driver.
//!
//! Chat completions, streaming, structured outputs, search (Responses API),
//! and tool use. Vendor wire translation is delegated to the shared
//! `systemprompt_models::wire` codecs via the `canonical_bridge`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod generation;
mod provider;
pub mod search;
mod streaming;
mod trait_impl;

pub use provider::OpenAiProvider;
