//! MCP-to-A2A bridging for tool execution within agent tasks: transforming
//! MCP tool results into A2A artifacts ([`artifact_transformer`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod artifact_transformer;

pub use artifact_transformer::{McpToA2aTransformer, infer_type, parse_wire_result};
