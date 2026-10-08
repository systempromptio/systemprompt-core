//! Bridge between the agent provider clients and the shared
//! [`systemprompt_wire`] codec.
//!
//! Each provider client builds a [`request::CanonicalBuild`] (which applies the
//! per-provider sampling/reasoning policy), hands the resulting
//! [`systemprompt_wire::canonical::CanonicalRequest`] to the protocol
//! codec to render the wire body, and maps the parsed
//! [`systemprompt_wire::canonical::CanonicalResponse`] back through the
//! `response` / `stream` submodules. Vendor wire translation lives in the
//! shared codec only; this module owns the agent-side glue (auto-policy,
//! canonical→agent type mapping) and nothing vendor-specific.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod request;
mod response;
mod stream;

pub use request::{BridgeProvider, CanonicalBuild, agent_response_format, tools_to_canonical};
pub use response::{
    CodeExecutionResponse, text_content, to_ai_response, to_code_execution, to_search_grounded,
    tool_calls,
};
pub use stream::event_to_chunk;
