//! Gemini provider driver.
//!
//! Chat completions, streaming, code-execution tool, Google Search grounding,
//! and tool use. Vendor wire translation is delegated to the shared
//! `systemprompt_models::wire::gemini` codec; this module keeps the transport,
//! the schema transformer / tool-name mapper, and the canonical glue.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod code_execution;
mod constants;
mod generation;
mod params;
mod provider;
mod search;
mod streaming;
mod tool_conversion;
mod tools;
mod trait_impl;
mod transport;

pub use code_execution::{CodeExecutionResponse, generate_with_code_execution};
pub use provider::GeminiProvider;
pub use tools::{ToolRequestParams, ToolResultParams};
